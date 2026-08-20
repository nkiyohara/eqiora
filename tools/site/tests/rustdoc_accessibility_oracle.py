#!/usr/bin/env python3
"""Fail-closed accessibility oracle for the published alpha.2 Rustdoc tree."""

from __future__ import annotations

import hashlib
import os
import stat
import sys
from dataclasses import dataclass, field
from html.parser import HTMLParser
from pathlib import Path
from typing import Iterable


EXPECTED_FILES = 2_214
EXPECTED_HTML = 1_377
EXPECTED_AFFECTED_HTML = 1_080
EXPECTED_TOGGLE_SUMMARIES = 93_115
EXPECTED_DIRECT_SECTIONS = 91_698
EXPECTED_SIGNATURE_LINKS = 268_082
EXPECTED_DESCRIPTION_LABELS = 1_417
EXPECTED_DIAGNOSTIC_STATIC_LINKS = 470
EXPECTED_DIAGNOSTIC_DETAILS = 107
EXPECTED_DIAGNOSTIC_DIRECT_SECTIONS = 106
EXPECTED_DIAGNOSTIC_SIGNATURE_LINKS = 321
EXPECTED_JAVASCRIPT_FILES = 808
EXPECTED_JAVASCRIPT_MANIFEST = (
    "fe57a6f030f576fec1cbd0e858e90dcf888d9bdb82d886c70760798394d743a2"
)
EXPECTED_SCRIPT_NODES = 5_610
EXPECTED_SCRIPT_MANIFEST = (
    "47257fa0bb9ce0c62fd3f4abbf156deedde98ce303ddc51f8521595a7a841b03"
)
EXPECTED_DIAGNOSTIC_HREF_ORDER = (
    "0efa4b5aba86cd16f7ad2938ed8502a6e9395cb0f766aa0a8200d153dda640d5"
)
MAIN_SCRIPT = Path("static.files/main-fcd733ba.js")
MAIN_SCRIPT_SHA256 = "baec8e8981b6e116315ea7ff1fed10b51352e001825228556ccd126317ed91db"
DIAGNOSTIC = Path("eqiora/struct.Diagnostic.html")
VOID_TAGS = frozenset(
    {
        "area",
        "base",
        "br",
        "col",
        "embed",
        "hr",
        "img",
        "input",
        "link",
        "meta",
        "param",
        "source",
        "track",
        "wbr",
    }
)
FORBIDDEN_SEMANTIC_ATTRIBUTES = frozenset(
    {"aria-hidden", "aria-label", "inert", "role", "tabindex"}
)
RAW_LINK_ATTRIBUTES = frozenset({"class", "data-notable-ty", "href", "title"})
PROJECTED_SPAN_ATTRIBUTES = frozenset(
    {"class", "data-eqiora-href", "data-notable-ty", "title"}
)


class OracleError(RuntimeError):
    """The supplied tree does not satisfy the frozen oracle."""


@dataclass
class Node:
    tag: str
    attrs: list[tuple[str, str | None]] = field(default_factory=list)
    children: list[Node | str] = field(default_factory=list)
    parent: Node | None = None

    def attr(self, name: str) -> str | None:
        matches = [value for key, value in self.attrs if key == name]
        if len(matches) > 1:
            raise OracleError(f"duplicate {name!r} attribute on <{self.tag}>")
        return matches[0] if matches else None

    def classes(self) -> set[str]:
        return set((self.attr("class") or "").split())

    def elements(self) -> Iterable[Node]:
        for child in self.children:
            if isinstance(child, Node):
                yield child

    def descendants(self) -> Iterable[Node]:
        for child in self.elements():
            yield child
            yield from child.descendants()


class TreeParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=False)
        self.root = Node("#document")
        self.stack = [self.root]

    def _append(self, node: Node) -> None:
        node.parent = self.stack[-1]
        self.stack[-1].children.append(node)

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        node = Node(tag, attrs)
        self._append(node)
        if tag not in VOID_TAGS:
            self.stack.append(node)

    def handle_startendtag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        self._append(Node(tag, attrs))

    def handle_endtag(self, tag: str) -> None:
        if len(self.stack) == 1 or self.stack[-1].tag != tag:
            current = self.stack[-1].tag
            raise OracleError(
                f"malformed HTML: closing </{tag}> while <{current}> is open"
            )
        self.stack.pop()

    def handle_data(self, data: str) -> None:
        self.stack[-1].children.append(data)

    def handle_entityref(self, name: str) -> None:
        self.stack[-1].children.append(f"&{name};")

    def handle_charref(self, name: str) -> None:
        self.stack[-1].children.append(f"&#{name};")

    def handle_comment(self, data: str) -> None:
        self.stack[-1].children.append(f"<!--{data}-->")

    def handle_decl(self, decl: str) -> None:
        self.stack[-1].children.append(f"<!{decl}>")

    def unknown_decl(self, data: str) -> None:
        self.stack[-1].children.append(f"<![{data}]>")

    def close(self) -> None:
        super().close()
        if len(self.stack) != 1:
            raise OracleError(f"malformed HTML: unclosed <{self.stack[-1].tag}>")


def parse_html(source: str) -> Node:
    parser = TreeParser()
    parser.feed(source)
    parser.close()
    return parser.root


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encode_parts(parts: Iterable[str]) -> bytes:
    encoded = bytearray()
    for part in parts:
        raw = part.encode("utf-8")
        encoded.extend(len(raw).to_bytes(8, "big"))
        encoded.extend(raw)
    return bytes(encoded)


def normalized_attrs(
    node: Node, *, projected: bool = False
) -> tuple[tuple[str, str], ...]:
    attrs: list[tuple[str, str]] = []
    for name, value in node.attrs:
        key = "href" if projected and name == "data-eqiora-href" else name
        attrs.append((key, "" if value is None else value))
    return tuple(sorted(attrs))


def require_attribute_shape(node: Node, allowed: frozenset[str], context: str) -> None:
    names = [name for name, _ in node.attrs]
    if len(names) != len(set(names)):
        raise OracleError(f"{context}: duplicate projected-link attribute")
    unexpected = sorted(set(names) - allowed)
    if unexpected:
        raise OracleError(
            f"{context}: unexpected projected-link attributes {unexpected}"
        )


def normalized_markup(item: Node | str) -> str:
    if isinstance(item, str):
        return f"T{len(item)}:{item}"
    attrs = "".join(
        f"A{len(key)}:{key}={len(value)}:{value}"
        for key, value in normalized_attrs(item)
    )
    children = "".join(normalized_markup(child) for child in item.children)
    return f"E{len(item.tag)}:{item.tag}[{attrs}]({children})"


def normalized_inner(node: Node) -> str:
    return "".join(normalized_markup(child) for child in node.children)


def text_content(node: Node) -> str:
    parts: list[str] = []
    for child in node.children:
        if isinstance(child, str):
            parts.append(child)
        else:
            parts.append(text_content(child))
    return "".join(parts)


def direct_child(node: Node, tag: str) -> list[Node]:
    return [child for child in node.elements() if child.tag == tag]


def following_element(node: Node) -> Node | None:
    if node.parent is None:
        return None
    found = False
    for child in node.parent.children:
        if child is node:
            found = True
        elif found and isinstance(child, Node):
            return child
    return None


def forbid_semantic_shortcuts(node: Node, context: str) -> None:
    attrs = {name for name, _ in node.attrs}
    bad = sorted(attrs & FORBIDDEN_SEMANTIC_ATTRIBUTES)
    if bad:
        raise OracleError(f"{context}: forbidden semantic shortcut attributes {bad}")
    style = (node.attr("style") or "").lower().replace(" ", "")
    forbidden_style = (
        "display:none",
        "visibility:hidden",
        "pointer-events:none",
        "clip-path:",
    )
    if any(value in style for value in forbidden_style):
        raise OracleError(f"{context}: concealed or silenced content")


@dataclass
class PageStats:
    toggle_summaries: int = 0
    direct_sections: int = 0
    raw_links: int = 0
    projected_links: int = 0
    projection_groups: int = 0
    hideme_labels: int = 0
    description_labels: int = 0
    raw_description_labels: int = 0
    active_hrefs: list[str] = field(default_factory=list)
    script_nodes: list[str] = field(default_factory=list)


def validate_projection_group(summary: Node, group: Node, context: str) -> int:
    if group.tag != "div" or "eqiora-signature-links" not in group.classes():
        raise OracleError(f"{context}: projected summary lacks its adjacent link group")
    forbid_semantic_shortcuts(group, context)
    elements = list(group.elements())
    if not elements:
        raise OracleError(f"{context}: projected link group is empty")
    label = elements[0]
    if label.tag != "span" or "eqiora-signature-links__label" not in label.classes():
        raise OracleError(f"{context}: projected link group lacks its visible label")
    if text_content(label).strip() != "Signature links:":
        raise OracleError(f"{context}: projected link group label drift")
    if label.attr("style") is not None:
        raise OracleError(f"{context}: projected link label has inline presentation")

    source_links = [
        node
        for node in summary.descendants()
        if node.tag == "span" and node.attr("data-eqiora-href") is not None
    ]
    clones = elements[1:]
    if not source_links or len(source_links) != len(clones):
        raise OracleError(f"{context}: projected source/active link count mismatch")
    for offset, (source, clone) in enumerate(zip(source_links, clones, strict=True)):
        if clone.tag != "a" or clone.attr("href") is None:
            raise OracleError(
                f"{context}: projected clone {offset} is not an active link"
            )
        forbid_semantic_shortcuts(source, context)
        forbid_semantic_shortcuts(clone, context)
        require_attribute_shape(source, PROJECTED_SPAN_ATTRIBUTES, context)
        require_attribute_shape(clone, RAW_LINK_ATTRIBUTES, context)
        if source.attr("id") is not None or clone.attr("id") is not None:
            raise OracleError(f"{context}: projected link carries a duplicate-prone id")
        if normalized_attrs(source, projected=True) != normalized_attrs(clone):
            raise OracleError(f"{context}: projected link {offset} attribute drift")
        if normalized_inner(source) != normalized_inner(clone):
            raise OracleError(
                f"{context}: projected link {offset} text/descendant drift"
            )
        if not text_content(source).strip() or not text_content(clone).strip():
            raise OracleError(f"{context}: projected link {offset} lost visible text")
    return len(source_links)


def inspect_page(root: Node, context: str) -> PageStats:
    stats = PageStats()
    all_nodes = list(root.descendants())
    stats.active_hrefs = [
        node.attr("href") or ""
        for node in all_nodes
        if node.tag == "a" and node.attr("href") is not None
    ]
    stats.script_nodes = [
        normalized_markup(node) for node in all_nodes if node.tag == "script"
    ]

    for details in (
        node
        for node in all_nodes
        if node.tag == "details" and "toggle" in node.classes()
    ):
        summaries = direct_child(details, "summary")
        if len(summaries) != 1:
            raise OracleError(
                f"{context}: toggle details does not have exactly one direct summary"
            )
        summary = summaries[0]
        stats.toggle_summaries += 1
        forbid_semantic_shortcuts(details, context)
        forbid_semantic_shortcuts(summary, context)
        sections = direct_child(summary, "section")
        if len(sections) > 1:
            raise OracleError(f"{context}: summary has multiple direct sections")
        raw_links = [
            node
            for node in summary.descendants()
            if node.tag == "a" and node.attr("href") is not None
        ]
        projected = [
            node
            for node in summary.descendants()
            if node.tag == "span" and node.attr("data-eqiora-href") is not None
        ]
        for link in raw_links:
            require_attribute_shape(link, RAW_LINK_ATTRIBUTES, context)
            if link.attr("id") is not None:
                raise OracleError(f"{context}: id-bearing nested active link")
            if any(child.tag == "a" for child in link.descendants()):
                raise OracleError(f"{context}: nested anchor shape")
        if sections:
            stats.direct_sections += 1
            if bool(raw_links) == bool(projected):
                raise OracleError(
                    f"{context}: summary is neither wholly raw nor wholly projected"
                )
            if raw_links:
                stats.raw_links += len(raw_links)
                if (
                    following_element(summary) is not None
                    and "eqiora-signature-links" in following_element(summary).classes()
                ):
                    raise OracleError(
                        f"{context}: raw summary has an existing projection marker"
                    )
            else:
                group = following_element(summary)
                if group is None:
                    raise OracleError(
                        f"{context}: projected summary has no adjacent element"
                    )
                stats.projected_links += validate_projection_group(
                    summary, group, context
                )
                stats.projection_groups += 1
        elif "hideme" in summary.classes():
            labels = direct_child(summary, "span")
            if len(labels) != 1:
                raise OracleError(f"{context}: hideme summary does not have one label")
            label = text_content(labels[0]).strip()
            if label == "Description":
                stats.description_labels += 1
            elif label == "Expand description":
                stats.raw_description_labels += 1
            elif not label:
                raise OracleError(f"{context}: hideme summary has an empty label")
            stats.hideme_labels += 1
            forbid_semantic_shortcuts(labels[0], context)
        else:
            raise OracleError(f"{context}: unexpected toggle-summary shape")
    all_groups = [
        node for node in all_nodes if "eqiora-signature-links" in node.classes()
    ]
    all_projected = [
        node for node in all_nodes if node.attr("data-eqiora-href") is not None
    ]
    if (
        len(all_groups) != stats.projection_groups
        or len(all_projected) != stats.projected_links
    ):
        raise OracleError(f"{context}: orphaned or misplaced projection marker")
    if any(node.tag != "span" for node in all_projected):
        raise OracleError(f"{context}: projection marker is not a span")
    return stats


def manifest_digest(entries: Iterable[tuple[str, bytes]]) -> str:
    parts: list[str] = []
    for path, payload in entries:
        parts.extend((path, sha256(payload)))
    return sha256(encode_parts(parts))


def inspect_tree(root: Path) -> tuple[PageStats, int, int, int, str, str]:
    if not root.is_dir() or root.is_symlink():
        raise OracleError("argument must be one real published Rustdoc directory")
    regular: list[Path] = []
    for directory, dirnames, filenames in os.walk(root, followlinks=False):
        base = Path(directory)
        for name in dirnames:
            if (base / name).is_symlink():
                raise OracleError(
                    f"symlink directory is forbidden: {(base / name).relative_to(root)}"
                )
        for name in filenames:
            path = base / name
            mode = path.lstat().st_mode
            if not stat.S_ISREG(mode) or path.is_symlink():
                raise OracleError(
                    f"non-regular output is forbidden: {path.relative_to(root)}"
                )
            regular.append(path)
    regular.sort(key=lambda path: path.relative_to(root).as_posix())
    html = [path for path in regular if path.suffix == ".html"]
    javascript = [path for path in regular if path.suffix == ".js"]
    if len(regular) != EXPECTED_FILES or len(html) != EXPECTED_HTML:
        raise OracleError(
            f"inventory drift: files/html={len(regular)}/{len(html)}, expected {EXPECTED_FILES}/{EXPECTED_HTML}"
        )
    if (root / "eqiora_mcp").exists():
        raise OracleError("unexpected eqiora_mcp output")
    main_script = root / MAIN_SCRIPT
    if (
        not main_script.is_file()
        or sha256(main_script.read_bytes()) != MAIN_SCRIPT_SHA256
    ):
        raise OracleError("upstream Rustdoc main script identity drift")
    js_digest = manifest_digest(
        (path.relative_to(root).as_posix(), path.read_bytes()) for path in javascript
    )
    if (
        len(javascript) != EXPECTED_JAVASCRIPT_FILES
        or js_digest != EXPECTED_JAVASCRIPT_MANIFEST
    ):
        raise OracleError("upstream JavaScript file inventory or bytes drift")

    total = PageStats()
    affected = 0
    script_entries: list[tuple[str, str]] = []
    diagnostic: PageStats | None = None
    for path in html:
        relative = path.relative_to(root).as_posix()
        page = inspect_page(parse_html(path.read_text(encoding="utf-8")), relative)
        total.toggle_summaries += page.toggle_summaries
        total.direct_sections += page.direct_sections
        total.raw_links += page.raw_links
        total.projected_links += page.projected_links
        total.projection_groups += page.projection_groups
        total.hideme_labels += page.hideme_labels
        total.description_labels += page.description_labels
        total.raw_description_labels += page.raw_description_labels
        if page.raw_links or page.projected_links:
            affected += 1
        script_entries.extend((relative, script) for script in page.script_nodes)
        if path.relative_to(root) == DIAGNOSTIC:
            diagnostic = page
    script_digest = sha256(
        encode_parts(part for entry in script_entries for part in entry)
    )
    if (
        len(script_entries) != EXPECTED_SCRIPT_NODES
        or script_digest != EXPECTED_SCRIPT_MANIFEST
    ):
        raise OracleError("HTML script-node inventory or bytes drift")
    if diagnostic is None:
        raise OracleError(f"missing representative item {DIAGNOSTIC.as_posix()}")
    validate_exact_counts(total, affected, diagnostic)
    return (
        total,
        affected,
        len(javascript),
        js_digest,
        script_digest,
        href_digest(diagnostic.active_hrefs),
    )


def href_digest(hrefs: list[str]) -> str:
    return sha256(encode_parts(hrefs))


def validate_exact_counts(
    total: PageStats, affected: int, diagnostic: PageStats
) -> None:
    actual = (
        affected,
        total.toggle_summaries,
        total.direct_sections,
        total.raw_links + total.projected_links,
        total.hideme_labels,
    )
    expected = (
        EXPECTED_AFFECTED_HTML,
        EXPECTED_TOGGLE_SUMMARIES,
        EXPECTED_DIRECT_SECTIONS,
        EXPECTED_SIGNATURE_LINKS,
        EXPECTED_DESCRIPTION_LABELS,
    )
    if actual != expected:
        raise OracleError(f"whole-tree shape drift: {actual!r} != {expected!r}")
    diagnostic_actual = (
        diagnostic.toggle_summaries,
        diagnostic.direct_sections,
        diagnostic.raw_links + diagnostic.projected_links,
        len(diagnostic.active_hrefs),
    )
    diagnostic_expected = (
        EXPECTED_DIAGNOSTIC_DETAILS,
        EXPECTED_DIAGNOSTIC_DIRECT_SECTIONS,
        EXPECTED_DIAGNOSTIC_SIGNATURE_LINKS,
        EXPECTED_DIAGNOSTIC_STATIC_LINKS,
    )
    if diagnostic_actual != diagnostic_expected:
        raise OracleError(
            f"Diagnostic shape drift: {diagnostic_actual!r} != {diagnostic_expected!r}"
        )
    if href_digest(diagnostic.active_hrefs) != EXPECTED_DIAGNOSTIC_HREF_ORDER:
        raise OracleError("Diagnostic active href order drift")
    if total.raw_links and total.projected_links:
        raise OracleError("partial whole-tree projection")
    if total.description_labels and total.raw_description_labels:
        raise OracleError("partial whole-tree description-label projection")
    if total.raw_links:
        raise OracleError(
            f"parent RED: {total.raw_links} active links remain nested in Rustdoc summaries"
        )
    if total.projected_links != EXPECTED_SIGNATURE_LINKS:
        raise OracleError("published tree is not completely projected")
    if total.projection_groups != EXPECTED_DIRECT_SECTIONS:
        raise OracleError("projected group count drift")
    if total.raw_description_labels:
        raise OracleError("generic top-description action label remains")


ACCESSIBLE_FIXTURE = """<!doctype html><html lang="en"><body><main><h1>Fixture</h1>
<details class="toggle top-doc" open><summary class="hideme"><span>Description</span></summary><div>An ordinary description.</div></details>
<details class="toggle method-toggle" open><summary><section id="method.fixture" class="method"><h2>pub fn <span class="fn" data-eqiora-href="#method.fixture">fixture</span>(value: <span class="struct" title="struct crate::Value" data-eqiora-href="struct.Value.html">Value</span>)</h2></section></summary><div class="eqiora-signature-links"><span class="eqiora-signature-links__label">Signature links:</span><a class="fn" href="#method.fixture">fixture</a><a class="struct" title="struct crate::Value" href="struct.Value.html">Value</a></div><div>Documentation.</div></details>
</main></body></html>"""


def expect_rejected(name: str, source: str) -> None:
    try:
        stats = inspect_page(parse_html(source), f"self-test {name}")
    except OracleError:
        return
    if stats.raw_links or stats.raw_description_labels:
        return
    raise OracleError(f"ordinary self-test mutant survived: {name}")


def run_ordinary_self_test() -> None:
    fixture = inspect_page(
        parse_html(ACCESSIBLE_FIXTURE), "ordinary accessible fixture"
    )
    if (
        fixture.toggle_summaries,
        fixture.direct_sections,
        fixture.projected_links,
        fixture.projection_groups,
        fixture.hideme_labels,
    ) != (2, 1, 2, 1, 1):
        raise OracleError(
            "ordinary accessible fixture did not reach its complete positive path"
        )
    mutations = {
        "one-nested": ACCESSIBLE_FIXTURE.replace(
            '<span class="fn" data-eqiora-href="#method.fixture">fixture</span>',
            '<a class="fn" href="#method.fixture">fixture</a>',
        ),
        "link-loss": ACCESSIBLE_FIXTURE.replace(
            '<a class="struct" title="struct crate::Value" href="struct.Value.html">Value</a>',
            "",
        ),
        "link-drift": ACCESSIBLE_FIXTURE.replace(
            '<a class="struct" title="struct crate::Value" href="struct.Value.html">',
            '<a class="struct" title="struct crate::Value" href="struct.Other.html">',
        ),
        "link-order": ACCESSIBLE_FIXTURE.replace(
            '<a class="fn" href="#method.fixture">fixture</a><a class="struct" title="struct crate::Value" href="struct.Value.html">Value</a>',
            '<a class="struct" title="struct crate::Value" href="struct.Value.html">Value</a><a class="fn" href="#method.fixture">fixture</a>',
        ),
        "link-extra": ACCESSIBLE_FIXTURE.replace(
            "</div><div>Documentation.",
            '<a class="fn" href="#method.fixture">fixture</a></div><div>Documentation.',
        ),
        "text-loss": ACCESSIBLE_FIXTURE.replace(
            'data-eqiora-href="#method.fixture">fixture</span>',
            'data-eqiora-href="#method.fixture"></span>',
        ),
        "role-spoof": ACCESSIBLE_FIXTURE.replace(
            "<summary><section", '<summary role="group"><section'
        ),
        "focus-silence": ACCESSIBLE_FIXTURE.replace(
            '<a class="fn" href="#method.fixture">',
            '<a class="fn" href="#method.fixture" tabindex="-1">',
        ),
        "hide": ACCESSIBLE_FIXTURE.replace(
            '<span class="fn" data-eqiora-href=',
            '<span class="fn" style="display:none" data-eqiora-href=',
        ),
        "section-move": ACCESSIBLE_FIXTURE.replace(
            "<summary><section", "<section"
        ).replace("</section></summary>", "</section><summary></summary>"),
        "description": ACCESSIBLE_FIXTURE.replace(
            ">Description</span>", ">Expand description</span>"
        ),
        "label-loss": ACCESSIBLE_FIXTURE.replace("Signature links:", "Links:"),
        "id-duplication": ACCESSIBLE_FIXTURE.replace(
            '<span class="fn" data-eqiora-href=',
            '<span id="duplicate" class="fn" data-eqiora-href=',
        ),
    }
    for name, source in mutations.items():
        expect_rejected(name, source)


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(
            "usage: rustdoc_accessibility_oracle.py PUBLISHED_RUSTDOC_ROOT",
            file=sys.stderr,
        )
        return 2
    try:
        run_ordinary_self_test()
        total, affected, javascript, js_digest, script_digest, diagnostic_hrefs = (
            inspect_tree(Path(argv[1]))
        )
    except (OSError, UnicodeError, OracleError) as error:
        print(f"rustdoc accessibility oracle: {error}", file=sys.stderr)
        return 1
    print(
        "rustdoc accessibility oracle: PASS "
        f"files/html/affected={EXPECTED_FILES}/{EXPECTED_HTML}/{affected} "
        f"summaries/sections/links/labels={total.toggle_summaries}/"
        f"{total.direct_sections}/{total.projected_links}/{total.hideme_labels} "
        f"javascript={javascript}:{js_digest} scripts={script_digest} "
        f"Diagnostic-hrefs={diagnostic_hrefs}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
