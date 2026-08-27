import re
from collections import Counter
from pathlib import Path, PurePosixPath
from urllib.parse import urlsplit

__all__ = ("RUSTDOC_ROOT", "check_rustdoc")

RUSTDOC_ROOT = Path("reference/rust/api")
BACK_FILES = {"help.html", "settings.html"}
_SITE_ABSOLUTE_TARGETS = {
    "/favicon.svg": Path("favicon.svg"),
    "/reference/rust/": Path("reference/rust/index.html"),
}
RAW_ID = re.compile(
    r"""(?<![\w:-])id\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'`=<>]+))""",
    re.IGNORECASE,
)


def _raw_ids(raw: str) -> Counter[str]:
    values = (
        next(value for value in match.groups() if value is not None)
        for match in RAW_ID.finditer(raw)
    )
    return Counter(values)


def _optional_generated_reference(source: str, value: str) -> bool:
    path = PurePosixPath(urlsplit(value).path)
    return (
        "trait.impl" in path.parts
        and path.name.startswith("trait.")
        and path.suffix == ".js"
    ) or (source in BACK_FILES and value == "./index.html")


def check_rustdoc(
    artifact: Path,
    inspections: dict[Path, tuple[str, object]],
) -> list[str]:
    root = artifact / RUSTDOC_ROOT
    errors: list[str] = []
    if inspections and root / "eqiora/index.html" not in inspections:
        errors.append("missing exact Rustdoc crate root eqiora/index.html")
    for path, (raw, _) in inspections.items():
        relative = path.relative_to(root)
        if relative.is_relative_to(Path("reference/rust/api")):
            errors.append("doubled Rustdoc public prefix")
        if re.search(r"<h1>Crate\s+([^<]+)</h1>", raw) and relative != Path(
            "eqiora/index.html"
        ):
            crate = relative.parts[0] if relative.parts else ""
            errors.append(f"unexpected Rustdoc crate root {crate!r}")
    strict = any(root / name in inspections for name in BACK_FILES)
    report = errors.append
    for path, (_, parser) in sorted(inspections.items()):
        candidates = []
        for tag, attrs, label in parser.interactives:
            handlers = " ".join(
                value for name, value in attrs.items() if name.startswith("on")
            )
            identity = label in {"Back", "Return"} or attrs.get("id") in {
                "back",
                "return",
            }
            if (
                identity
                or attrs.get("href", "").startswith("javascript:")
                or "history." in handlers
            ):
                candidates.append((tag, attrs, label))
        relative = path.relative_to(root).as_posix()
        expected = strict and relative in BACK_FILES
        valid = len(candidates) == 1 if expected else not candidates
        if expected and valid:
            tag, attrs, label = candidates[0]
            valid = (
                tag == "a"
                and label == "Back"
                and attrs.get("id") == "back"
                and attrs.get("href") == "javascript:void(0)"
                and attrs.get("onclick") == "history.back();"
                and not any(
                    name.startswith("on") and name != "onclick" for name in attrs
                )
            )
        if not valid:
            errors.append(f"{relative}: invalid generated Rustdoc Back control")
    if strict:
        for relative in BACK_FILES:
            if root / relative not in inspections:
                errors.append(f"{relative}: invalid generated Rustdoc Back control")
    raw_ids = {path: _raw_ids(raw) for path, (raw, _) in inspections.items()}
    for source, (_, parser) in sorted(inspections.items()):
        source_name = source.relative_to(root).as_posix()
        for tag, attribute, value in parser.references:
            parsed = urlsplit(value)
            if parsed.scheme in {"http", "https", "mailto", "tel"}:
                continue
            if value == "javascript:void(0)" and tag == "a":
                continue
            if parsed.scheme or value.startswith("//"):
                report(f"{source_name}: unsafe Rustdoc reference {value!r}")
                continue
            if parsed.path.startswith("/"):
                relative_target = _SITE_ABSOLUTE_TARGETS.get(value)
                if relative_target is not None:
                    target = artifact / relative_target
                    if target.is_symlink() or not target.is_file():
                        report(
                            f"{source_name}: Rustdoc target has wrong type {value!r}"
                        )
                    continue
                target = artifact / parsed.path.lstrip("/")
            else:
                target = source.parent / parsed.path if parsed.path else source
            target = target.resolve()
            try:
                target.relative_to(root)
            except ValueError:
                report(f"{source_name}: Rustdoc reference escapes exact root {value!r}")
                continue
            if target.exists():
                if target.is_symlink() or not target.is_file():
                    report(f"{source_name}: Rustdoc target has wrong type {value!r}")
                elif parsed.fragment:
                    target_ids = raw_ids.get(target, Counter())
                    count = target_ids[parsed.fragment]
                    if count == 0:
                        line_range = re.fullmatch(
                            r"([1-9][0-9]*)-([1-9][0-9]*)", parsed.fragment
                        )
                        if line_range and (len(line_range[1]), line_range[1]) < (
                            len(line_range[2]),
                            line_range[2],
                        ):
                            endpoints = (line_range[1], line_range[2])
                            counts = tuple(
                                target_ids[endpoint] for endpoint in endpoints
                            )
                            if counts == (1, 1):
                                continue
                            if 0 not in counts:
                                duplicate = endpoints[counts.index(max(counts))]
                                report(
                                    f"{source_name}: duplicate Rustdoc target ID {duplicate!r}"
                                )
                                continue
                        report(
                            f"{source_name}: missing raw Rustdoc fragment target {value!r}"
                        )
                    elif count > 1:
                        report(
                            f"{source_name}: duplicate Rustdoc target ID {parsed.fragment!r}"
                        )
                continue
            if _optional_generated_reference(source_name, value):
                continue
            if attribute in {"href", "src"}:
                report(f"{source_name}: unadmitted missing Rustdoc reference {value!r}")
    return errors
