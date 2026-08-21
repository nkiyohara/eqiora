from html.parser import HTMLParser
from pathlib import Path

__all__ = ("HtmlInspection", "normalize", "read_html")


def normalize(text: str) -> str:
    return " ".join(text.split())


class HtmlInspection(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.stack: list[tuple[str, dict[str, str], bool]] = []
        self.collectors: list[list[object]] = []
        self.text: list[str] = []
        self.headings: list[tuple[int, str]] = []
        self.anchors: list[tuple[str, str]] = []
        self.interactives: list[tuple[str, dict[str, str], str]] = []
        self.images: list[dict[str, str]] = []
        self.metas: list[dict[str, str]] = []
        self.links: list[dict[str, str]] = []
        self.references: list[tuple[str, str, str]] = []
        self.id_text: dict[str, list[str]] = {}
        self.math: list[tuple[str, bool]] = []
        self.inline_handlers: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        values = {name.lower(): value or "" for name, value in attrs}
        parent_hidden = self.stack[-1][2] if self.stack else False
        hidden = (
            parent_hidden
            or tag in {"script", "style", "template", "svg"}
            or "hidden" in values
            or values.get("aria-hidden") == "true"
        )
        self.stack.append((tag, values, hidden))
        if identifier := values.get("id"):
            self.id_text.setdefault(identifier, [])
        for name in values:
            if name.startswith("on"):
                self.inline_handlers.append(f"{tag}[{name}]")
        if tag in {"h1", "h2", "h3", "h4", "h5", "h6", "a", "button"} or values.get(
            "role"
        ) in {"button", "link"}:
            self.collectors.append([tag, values, []])
        if tag == "img":
            if not hidden and values.get("alt"):
                for collector in self.collectors:
                    if collector[0] not in {"h1", "h2", "h3", "h4", "h5", "h6"}:
                        collector[2].extend((" ", values["alt"], " "))
            for ancestor, ancestor_values, _ in reversed(self.stack[:-1]):
                if ancestor == "a":
                    values["_ancestor_href"] = ancestor_values.get("href", "")
                    break
            self.images.append(values)
        if tag == "meta":
            self.metas.append(values)
        if tag == "link":
            self.links.append(values)
        if tag == "input" and values.get("type", "text").lower() in {
            "button",
            "submit",
            "reset",
            "image",
        }:
            self.interactives.append(
                (
                    tag,
                    values,
                    values.get("value") or values.get("alt") or values["type"].lower(),
                )
            )
        if tag == "math":
            in_display = any(
                "katex-display" in item[1].get("class", "").split()
                for item in self.stack[:-1]
            )
            self.math.append((values.get("display", "inline"), in_display))
        for attribute in ("href", "src", "poster", "action", "data", "style"):
            if attribute in values:
                self.references.append((tag, attribute, values[attribute]))
        if "srcset" in values:
            for item in values["srcset"].split(","):
                self.references.append((tag, "srcset", item.strip().split()[0]))
        if "katex-display" in values.get("class", "").split() and tag not in {
            "div",
            "span",
        }:
            self.references.append((tag, "invalid-katex-display-owner", ""))

    def handle_endtag(self, tag: str) -> None:
        for index in range(len(self.collectors) - 1, -1, -1):
            collector = self.collectors[index]
            if collector[0] != tag:
                continue
            _, attrs, chunks = self.collectors.pop(index)
            collected = normalize("".join(chunks))
            if tag.startswith("h") and len(tag) == 2 and tag[1].isdigit():
                self.headings.append((int(tag[1]), collected))
            elif tag == "a":
                self.anchors.append((str(attrs.get("href", "")), collected))
                self.interactives.append((tag, attrs, collected))
            else:
                self.interactives.append((tag, attrs, collected))
            break
        for index in range(len(self.stack) - 1, -1, -1):
            if self.stack[index][0] == tag:
                del self.stack[index:]
                break

    def handle_data(self, data: str) -> None:
        for _, attrs, _ in self.stack:
            if identifier := attrs.get("id"):
                self.id_text[identifier].append(data)
        if not (self.stack and self.stack[-1][2]):
            self.text.append(data)
            for collector in self.collectors:
                collector[2].append(data)

    @property
    def visible_text(self) -> str:
        return normalize("".join(self.text))


def read_html(path: Path, max_bytes: int) -> tuple[str, HtmlInspection]:
    if path.stat().st_size > max_bytes:
        raise ValueError(f"HTML exceeds {max_bytes} bytes")
    text = path.read_text(encoding="utf-8")
    parser = HtmlInspection()
    parser.feed(text)
    parser.close()
    return text, parser
