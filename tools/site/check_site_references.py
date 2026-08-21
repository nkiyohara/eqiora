"""Private bounded HTML/CSS reference policy for the site artifact."""

from __future__ import annotations

import base64
import binascii
import re
from pathlib import Path
from urllib.parse import unquote, unquote_to_bytes, urlsplit
from xml.etree import ElementTree

__all__ = ("check_references",)

SITE_ORIGIN = "https://eqiora.org"
MAX_DATA_URL_BYTES = 1_048_576
MAX_HTML_REFERENCES = 1_000_000
MAX_CSS_URLS = 4_096
CSS_URL_TOKEN = re.compile("url", re.IGNORECASE | re.ASCII)
LEGACY_OBSERVER_PIXEL = "data:image/gif;base64,R0lGODlhAQABAAAAACw="
RUNTIME_TAGS = {
    "script",
    "img",
    "source",
    "iframe",
    "video",
    "audio",
    "track",
    "embed",
    "object",
}


def _runtime_reference(tag: str, attribute: str) -> bool:
    return (
        (tag in RUNTIME_TAGS and attribute in {"src", "srcset", "poster", "data"})
        or (tag == "link" and attribute == "href")
        or (tag == "form" and attribute == "action")
        or attribute == "style"
    )


def _local_reference(
    artifact: Path,
    page: Path,
    reference: str,
) -> tuple[Path | None, str]:
    parsed = urlsplit(reference)
    if parsed.scheme or reference.startswith("//"):
        if (
            parsed.scheme in {"http", "https"}
            and f"{parsed.scheme}://{parsed.netloc}" == SITE_ORIGIN
        ):
            raw_path = unquote(parsed.path)
        else:
            return None, parsed.fragment
    else:
        raw_path = unquote(parsed.path)
    if not raw_path:
        return page, parsed.fragment
    target = (
        artifact / raw_path.lstrip("/")
        if raw_path.startswith("/")
        else page.parent / raw_path
    )
    target = target.resolve()
    try:
        target.relative_to(artifact)
    except ValueError:
        return Path("/__escape__"), parsed.fragment
    if target.is_dir() or (target.suffix == "" and not target.exists()):
        target /= "index.html"
    return target, parsed.fragment


def _css_urls(text: str) -> tuple[list[str], list[str]]:
    values: list[str] = []
    errors: list[str] = []
    offset = 0
    while offset < len(text):
        match = CSS_URL_TOKEN.search(text, offset)
        if match is None:
            break
        start = match.start()
        cursor = start + 3
        while cursor < len(text) and text[cursor].isspace():
            cursor += 1
        if cursor >= len(text) or text[cursor] != "(":
            offset = cursor
            continue
        cursor += 1
        while cursor < len(text) and text[cursor].isspace():
            cursor += 1
        quote = text[cursor] if cursor < len(text) and text[cursor] in "\"'" else ""
        if quote:
            cursor += 1
        value_start = cursor
        closed = False
        while cursor < len(text):
            character = text[cursor]
            if character == "\\":
                cursor += 2
                continue
            if quote and character == quote:
                value_end = cursor
                cursor += 1
                while cursor < len(text) and text[cursor].isspace():
                    cursor += 1
                closed = cursor < len(text) and text[cursor] == ")"
                break
            if not quote and character == ")":
                value_end = cursor
                closed = True
                break
            cursor += 1
        if not closed:
            errors.append("unterminated CSS url()")
            break
        values.append(text[value_start:value_end].strip())
        if len(values) > MAX_CSS_URLS:
            errors.append(f"CSS file exceeds {MAX_CSS_URLS} URL references")
            break
        offset = cursor + 1
    return values, errors


def _decode_data_url(value: str) -> tuple[str | None, bytes | None, list[str]]:
    raw_oversize = len(value.encode("utf-8")) > MAX_DATA_URL_BYTES
    try:
        header, payload = value[5:].split(",", 1)
    except ValueError:
        return None, None, ["malformed CSS data URL"]
    parts = header.split(";")
    media = parts[0].casefold()
    encoded = parts[-1].casefold() == "base64"
    try:
        if encoded:
            if parts[1:] != ["base64"]:
                raise ValueError
            decoded = base64.b64decode(payload, validate=True)
        else:
            if parts[1:] or re.search(r"%(?![0-9a-fA-F]{2})", payload):
                raise ValueError
            decoded = unquote_to_bytes(payload)
    except (ValueError, binascii.Error):
        return media, None, ["malformed CSS data URL"]
    if len(decoded) > MAX_DATA_URL_BYTES:
        return media, None, [f"decoded data URL exceeds {MAX_DATA_URL_BYTES} bytes"]
    if raw_oversize:
        return media, None, [f"raw data URL exceeds {MAX_DATA_URL_BYTES} bytes"]
    return media, decoded, []


def _check_svg(payload: bytes) -> list[str]:
    upper = payload.upper()
    if b"<!DOCTYPE" in upper or b"<!ENTITY" in upper:
        return ["SVG data URL contains active content"]
    try:
        root = ElementTree.fromstring(payload)
    except ElementTree.ParseError:
        return ["malformed CSS data URL"]
    active = {
        "script",
        "foreignObject",
        "animate",
        "animateMotion",
        "animateTransform",
        "set",
        "style",
    }
    for node in root.iter():
        if node.tag.rsplit("}", 1)[-1] in active:
            return ["SVG data URL contains active content"]
        for name, value in node.attrib.items():
            local = name.rsplit("}", 1)[-1].casefold()
            if local.startswith("on") or (local == "href" and value.strip()):
                return ["SVG data URL contains active content"]
    return []


def _check_data_url(value: str) -> list[str]:
    media, payload, errors = _decode_data_url(value)
    if errors:
        return errors
    if media not in {"font/woff2", "image/svg+xml"}:
        return ["CSS data URL media type is not admitted"]
    assert payload is not None
    if media == "font/woff2" and not payload.startswith(b"wOF2"):
        return ["WOFF2 data URL lacks the wOF2 signature"]
    if media == "image/svg+xml":
        return _check_svg(payload)
    return []


def _check_html(
    artifact: Path,
    inspections: dict[Path, tuple[str, object]],
    references: list[tuple[Path, str, str, str]],
    source_sha: str,
) -> list[str]:
    errors: list[str] = []
    report = errors.append
    parsed = {path: value[1] for path, value in inspections.items()}
    for page_path, tag, attribute, value in references:
        if page_path not in parsed:
            continue
        if not value:
            report(f"{page_path.relative_to(artifact)}: empty {attribute} reference")
            continue
        if value.startswith(("mailto:", "tel:")):
            continue
        if value.startswith("data:"):
            if value != LEGACY_OBSERVER_PIXEL:
                report(
                    f"{page_path.relative_to(artifact)}: data URL is forbidden in HTML"
                )
            continue
        target_url = urlsplit(value)
        if target_url.scheme not in {"", "http", "https"} or value.startswith("//"):
            report(f"{page_path.relative_to(artifact)}: unsafe reference {value!r}")
            continue
        if (
            target_url.scheme
            and f"{target_url.scheme}://{target_url.netloc}" != SITE_ORIGIN
        ):
            if re.match(
                r"^/nkiyohara/eqiora/(?:blob|tree)/", target_url.path
            ) and not re.match(
                rf"^https://github\.com/nkiyohara/eqiora/(?:blob|tree)/{source_sha}/",
                value,
            ):
                report(
                    f"{page_path.relative_to(artifact)}: repository source link does not use the exact asserted SHA: {value!r}"
                )
            if _runtime_reference(tag, attribute):
                report(
                    f"{page_path.relative_to(artifact)}: external runtime request {value!r}"
                )
            continue
        target, fragment = _local_reference(artifact, page_path, value)
        if target is None:
            continue
        if target == Path("/__escape__") or target.is_symlink() or not target.is_file():
            report(
                f"{page_path.relative_to(artifact)}: broken or escaping link {value!r}"
            )
            continue
        if fragment and target.suffix == ".html":
            target_parser = parsed.get(target)
            if target_parser is None or unquote(fragment) not in target_parser.id_text:
                report(
                    f"{page_path.relative_to(artifact)}: missing fragment target {value!r}"
                )
    return errors


def _check_css(artifact: Path, files: list[Path]) -> list[str]:
    errors: list[str] = []
    katex_woff2 = 0
    for stylesheet in sorted(
        path for path in files if path.suffix.casefold() == ".css"
    ):
        try:
            css = stylesheet.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        values, parser_errors = _css_urls(css)
        errors.extend(
            f"{stylesheet.relative_to(artifact)}: {error}" for error in parser_errors
        )
        for value in values[:MAX_CSS_URLS]:
            if value.startswith("data:"):
                errors.extend(
                    f"{stylesheet.relative_to(artifact)}: {error}"
                    for error in _check_data_url(value)
                )
                continue
            target, _ = _local_reference(artifact, stylesheet, value)
            if target is None or target.is_symlink() or not target.is_file():
                errors.append(
                    f"{stylesheet.relative_to(artifact)}: CSS asset is external or missing: {value!r}"
                )
            elif target.suffix.casefold() == ".woff2" and "katex" in css.casefold():
                katex_woff2 += 1
    if katex_woff2 == 0:
        errors.append("assembled KaTeX CSS exposes no resolvable local WOFF2 fonts")
    return errors


def check_references(
    artifact: Path,
    files: list[Path],
    all_inspections: dict[Path, tuple[str, object]],
    starlight_inspections: dict[Path, tuple[str, object]],
    source_sha: str,
) -> list[str]:
    references: list[tuple[Path, str, str, str]] = []
    style_errors: list[str] = []
    for page_path, (_, parser) in all_inspections.items():
        for tag, attribute, value in parser.references:
            if attribute == "style":
                values, errors = _css_urls(value)
                references.extend((page_path, tag, attribute, item) for item in values)
                style_errors += [
                    f"{page_path.relative_to(artifact)}: inline CSS: {error}"
                    for error in errors
                ]
            else:
                references.append((page_path, tag, attribute, value))
            if len(references) > MAX_HTML_REFERENCES:
                return [
                    f"artifact exceeds {MAX_HTML_REFERENCES} aggregate HTML references"
                ]
    return [
        *style_errors,
        *_check_html(artifact, starlight_inspections, references, source_sha),
        *_check_css(artifact, files),
    ]
