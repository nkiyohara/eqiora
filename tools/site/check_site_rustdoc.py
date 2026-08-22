import re
from collections import Counter
from pathlib import Path
from urllib.parse import urlsplit

__all__ = ("RUSTDOC_ROOT", "check_rustdoc")

RUSTDOC_ROOT = Path("reference/rust/api")
BACK_FILES = {"help.html", "settings.html"}
_SITE_ABSOLUTE_TARGETS = {
    "/favicon.svg": Path("favicon.svg"),
    "/reference/rust/": Path("reference/rust/index.html"),
}
ABSENT_REFERENCES = (
    (
        "eqiora/api/trait.ReferenceRunObserver.html",
        "../../trait.impl/eqiora_api/reference_run/trait.ReferenceRunObserver.js",
    ),
    (
        "eqiora/api/trait.ScalarEllipticRunObserver.html",
        "../../trait.impl/eqiora_api/spatial/plan/trait.ScalarEllipticRunObserver.js",
    ),
    (
        "eqiora/backends/mpi/trait.MpiRankLocalCsrAction.html",
        "../../../trait.impl/eqiora_backend_mpi/runtime/trait.MpiRankLocalCsrAction.js",
    ),
    (
        "eqiora/device/trait.CommandQueue.html",
        "../../trait.impl/eqiora_device/queue/trait.CommandQueue.js",
    ),
    (
        "eqiora/device/trait.DeviceBuffer.html",
        "../../trait.impl/eqiora_device/buffer/trait.DeviceBuffer.js",
    ),
    (
        "eqiora/device/trait.Fence.html",
        "../../trait.impl/eqiora_device/queue/trait.Fence.js",
    ),
)


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
    strict = any(root / name in inspections for name in BACK_FILES) or any(
        root / source in inspections for source, _ in ABSENT_REFERENCES
    )
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
    expected = Counter(ABSENT_REFERENCES if strict else ())
    if strict:
        expected.update(
            {("help.html", "./index.html"): 2, ("settings.html", "./index.html"): 2}
        )
    observed: Counter[tuple[str, str]] = Counter()
    expected_values = {value for _, value in expected}
    raw_ids = {
        path: Counter(re.findall(r'\bid=["\']([^"\']+)["\']', raw))
        for path, (raw, _) in inspections.items()
    }
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
            key = (source_name, value)
            if key not in expected and value in expected_values:
                report(
                    f"{source_name}: absent Rustdoc reference has wrong source {value!r}"
                )
                continue
            if parsed.path.startswith("/"):
                relative_target = _SITE_ABSOLUTE_TARGETS.get(value)
                if relative_target is not None:
                    target = artifact / relative_target
                    if target.is_symlink() or not target.is_file():
                        report(f"{source_name}: Rustdoc target has wrong type {value!r}")
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
                if key in expected:
                    report(
                        f"{source_name}: admitted absent Rustdoc target exists with wrong type"
                    )
                elif target.is_symlink() or not target.is_file():
                    report(f"{source_name}: Rustdoc target has wrong type {value!r}")
                elif parsed.fragment:
                    count = raw_ids.get(target, Counter())[parsed.fragment]
                    if count == 0:
                        report(
                            f"{source_name}: missing raw Rustdoc fragment target {value!r}"
                        )
                    elif count > 1:
                        report(
                            f"{source_name}: duplicate Rustdoc target ID {parsed.fragment!r}"
                        )
                continue
            if key in expected:
                observed[key] += 1
            elif attribute in {"href", "src"}:
                report(f"{source_name}: unadmitted missing Rustdoc reference {value!r}")
    for key, count in expected.items():
        if observed[key] != count:
            report(
                f"{key[0]}: missing expected absent Rustdoc reference occurrence {key[1]!r}"
            )
    return errors
