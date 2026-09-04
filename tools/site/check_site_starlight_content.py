import re
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit

__all__ = (
    "PRESSURE_ALT",
    "PRESSURE_CAPTION",
    "CASE_SOURCE_PATHS",
    "CASE_EVIDENCE_PATHS",
    "check_starlight_content",
)

PRESSURE_ALT = "Pressure in pascals for a 2D steady-Stokes exact-cylinder demonstration, shown with a viridis color scale and its current Gmsh mesh overlaid. Presentation image only; no numerical or mesh-output oracle."
PRESSURE_CAPTION = "Pressure (Pa), frozen exact-cylinder steady-Stokes demonstration at cd1185b0f8ec8940352e7b6bc832fd4ebe67591b; presentation only, not validation."
PUBLIC_CLAIM = "One presentation-only 2D steady incompressible Stokes exact-cylinder demonstration rendered through exact Geometry, typed Gmsh policy, and the root Result path; output counts, digests, numerical values, and pixels are not independently verified."
WITNESS_COPY = "The current Gmsh output is presentation input, not a fixed mesh or scientific oracle."
RENDERED_SOURCE_SENTENCE = "This website is a curated projection, not a parallel specification. Detailed contracts remain in the repository’s architecture, RFCs, capability matrix, and validated verify manifests."
REFERENCE_BOUNDARY = "API presence is neither capability evidence nor maturity."
TEXTBOOK_SERIES = (
    ("circuits-dynamics-hybrid", "Circuits, Dynamics, and Hybrid Systems"),
    ("fluid-mechanics-cfd", "Fluid Mechanics and Computational Fluid Dynamics"),
    ("heat-mass-transfer", "Heat and Mass Transfer"),
    ("mathematical-modeling", "Mathematical Modeling with Eqiora"),
    ("numerical-simulation", "Numerical Simulation with Eqiora"),
    ("structural-mechanics-fem", "Structural Mechanics and the Finite Element Method"),
)
MODELING_FOUNDATION_CHAPTERS = (
    ("algebraic-relations-networks", "Algebraic relations and networks", "Illustrative"),
    ("boundary-interface-conditions", "Boundary and interface conditions", "Illustrative"),
    ("conservation-laws", "Conservation laws", "Illustrative"),
    ("constitutive-laws", "Constitutive laws", "Illustrative"),
    ("fields-spatial-domains", "Fields and spatial domains", "Illustrative"),
    ("models-not-simulations", "Models are not simulations", "Illustrative"),
    ("ordinary-differential-equations", "Ordinary differential equations", "Checked"),
    ("quantities-dimensions-units", "Quantities, dimensions, and units", "Illustrative"),
)
STAGES = (
    ("problem-setup", "1", "Problem setup"),
    ("model-definition", "2", "Eqiora model definition"),
    ("mesh-and-boundaries", "3", "Mesh and boundaries"),
    ("submit-and-result", "4", "Submit and result"),
    ("pressure-visualization", "5", "Pressure visualization"),
    ("verified-boundary", "6", "Verification boundary"),
)
NONCLAIMS = (
    "This is a bounded 2D steady Stokes demonstration, not a transient-flow, convergence, force-coefficient, or performance benchmark.",
    "The current geometry and meshing path do not generalize to arbitrary providers, 3D, curved, boundary-layer, or adaptive meshes.",
    "Rendered values and pixels are illustrative output rather than validation data.",
)
ADMITTED_SOURCE_PATH = "examples/python/exact_cylinder_stokes.py"
ADMITTED_SOURCE_FRAGMENT = "#L45-L57"
ADMITTED_SOURCE_LABEL = "Eqiora source form: canonical Python resolve/run path"
CASE_SOURCE_PATHS = (
    ADMITTED_SOURCE_PATH,
    "examples/python/exact_cylinder_geometry.py",
    "examples/python/exact_cylinder_mesh.py",
    "verify/fluid/packaged-steady-stokes-2d/models/direct.eqi",
    "verify/fluid/packaged-steady-stokes-2d/package-v0.1.0/src/incompressible.eqi",
    "packages/Eqiora.Fluid.Incompressible/src/incompressible.eqi",
)
CASE_EVIDENCE_PATHS = (
    "verify/artifacts/current-model-canonical-identity/README.md",
    "verify/fluid/packaged-steady-stokes-2d/README.md",
    "verify/geometry/exact-circular-hole-geometry/README.md",
    "verify/interfaces/python-exact-circular-hole-geometry/README.md",
)
HOME_COPY = (
    "Model meaning once. Realize it many ways.",
    "Eqiora is an open-source, meaning-first foundation for scientific modeling, simulation, differentiation, and execution.",
    "Its central boundary is simple:",
    "A model states typed mathematical relations. A realization chooses how those relations are discretized, solved, and executed.",
    "That separation lets block diagrams, acausal physical networks, PDE fields, hybrid dynamics, and reusable components share one canonical meaning without making a numerical method or hardware backend part of the model.",
    "Get started",
    "Explore gallery",
    "Featured walkthrough",
    "Exact-cylinder steady Stokes",
    "Follow one frozen 2D steady-Stokes problem from model definition and named boundaries through one submit/Result path to an independently admitted static pressure image.",
    "Python",
    "2D",
    "steady Stokes",
    "View the static walkthrough",
    "Docs",
    "Learn the Model–Realization boundary and start from bounded examples.",
    "Textbooks",
    "Follow the planned path from mathematics and physics to Eqiora models, numerical realization, and interpretation.",
    "Capabilities",
    "See what is available, executable, checked, or verified.",
    "Reference",
    "Browse exact-commit Python, Rust, CLI, control-v2, and MCP surfaces.",
    "Docs explains how to use Eqiora. Textbooks teach the mathematics, physics, and numerics. Gallery presents complete simulations. Reference records exact APIs and protocols. Capabilities states what runs and the boundary of each claim.",
    "{release_identity}",
    "Eqiora is alpha research software under active development. The capability matrix and generated evidence catalog bound what is currently supported; this site does not widen those claims.",
    "One source of truth",
    RENDERED_SOURCE_SENTENCE,
)
EXECUTION_CONTROL = re.compile(
    r"\b(?:run|submit|reset|start|begin|try|solv\w*|execut\w*|simulat\w*|comput\w*|calculat\w*|launch\w*|evaluat\w*|process\w*|generat\w*|analy[sz]\w*|predict\w*)\b",
    re.IGNORECASE,
)


class _ContentInspection(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.stages: list[tuple[str, str, str]] = []
        self.math: list[dict[str, list[str]]] = []
        self.math_stack: list[dict[str, list[str]]] = []
        self.annotations: list[tuple[dict[str, list[str]], list[str]] | None] = []
        self.katex_mathml = 0
        self.katex_html = 0
        self.elements: list[tuple[str, bool, str]] = []
        self.active_ids: list[str] = []
        self.id_accessible_text: dict[str, list[str]] = {}
        self.id_attrs: dict[str, dict[str, str]] = {}

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        values = {name.casefold(): value or "" for name, value in attrs}
        classes = values.get("class", "").split()
        parent_hidden = self.elements[-1][1] if self.elements else False
        hidden = (
            parent_hidden
            or tag in {"script", "style", "template", "svg"}
            or "hidden" in values
            or values.get("aria-hidden") == "true"
        )
        if not hidden:
            for identifier in self.active_ids:
                self.id_accessible_text[identifier].append(" ")
        identifier = values.get("id", "")
        if identifier:
            self.id_accessible_text.setdefault(identifier, [])
            self.id_attrs[identifier] = values
            self.active_ids.append(identifier)
        self.elements.append((tag, hidden, identifier))
        if "katex-mathml" in classes:
            self.katex_mathml += 1
        if "katex-html" in classes:
            self.katex_html += 1
        if tag == "section" and "eq-stage" in classes:
            self.stages.append(
                (
                    values.get("id", ""),
                    values.get("data-step", ""),
                    values.get("aria-labelledby", ""),
                )
            )
        if tag == "math":
            record: dict[str, list[str]] = {"annotations": []}
            self.math.append(record)
            self.math_stack.append(record)
        elif tag == "annotation":
            if self.math_stack and values.get("encoding") == "application/x-tex":
                self.annotations.append((self.math_stack[-1], []))
            else:
                self.annotations.append(None)

    def handle_endtag(self, tag: str) -> None:
        if tag == "annotation" and self.annotations:
            current = self.annotations.pop()
            if current is not None:
                record, chunks = current
                record["annotations"].append(" ".join("".join(chunks).split()))
        elif tag == "math" and self.math_stack:
            self.math_stack.pop()
        for index in range(len(self.elements) - 1, -1, -1):
            if self.elements[index][0] != tag:
                continue
            removed = self.elements[index:]
            if not self.elements[index][1]:
                for identifier in self.active_ids:
                    self.id_accessible_text[identifier].append(" ")
            del self.elements[index:]
            for _, _, identifier in reversed(removed):
                if identifier:
                    self.active_ids.remove(identifier)
            break

    def handle_data(self, data: str) -> None:
        if self.annotations and self.annotations[-1] is not None:
            current = self.annotations[-1]
            assert current is not None
            current[1].append(data)
        if not (self.elements and self.elements[-1][1]):
            for identifier in self.active_ids:
                self.id_accessible_text[identifier].append(data)

    def accessible_name(self, identifier: str, seen: tuple[str, ...] = ()) -> str:
        if identifier in seen:
            return ""
        attrs = self.id_attrs.get(identifier, {})
        labelled = attrs.get("aria-labelledby", "").split()
        if labelled:
            value = " ".join(
                self.accessible_name(item, (*seen, identifier)) for item in labelled
            )
        elif "aria-label" in attrs:
            value = attrs["aria-label"]
        else:
            value = "".join(self.id_accessible_text.get(identifier, []))
        return " ".join(value.split())


def _same_origin_file(artifact: Path, page: Path, value: str) -> Path | None:
    parsed = urlsplit(value)
    if parsed.query or parsed.fragment or parsed.username or parsed.password:
        return None
    if parsed.scheme or value.startswith("//"):
        if (
            parsed.scheme not in {"http", "https"}
            or f"{parsed.scheme}://{parsed.netloc}" != "https://eqiora.org"
        ):
            return None
    raw_path = unquote(parsed.path)
    target = (
        artifact / raw_path.lstrip("/")
        if raw_path.startswith("/")
        else page.parent / raw_path
    )
    if target.is_symlink():
        return None
    target = target.resolve()
    try:
        target.relative_to(artifact)
    except ValueError:
        return None
    return target if target.is_file() else None


def _check_pressure_image(
    artifact: Path,
    page_path: Path,
    page: object,
    file_digests: dict[Path, str],
    pressure_digest: str,
    label: str,
) -> list[str]:
    resolved = [
        (image, _same_origin_file(artifact, page_path, image.get("src", "")))
        for image in page.images
    ]
    candidates = [
        (image, target)
        for image, target in resolved
        if image.get("alt") == PRESSURE_ALT
        or (target is not None and file_digests.get(target) == pressure_digest)
    ]
    if len(candidates) != 1:
        qualifier = "exactly one " if len(candidates) > 1 else "the "
        return [
            f"{label} must expose {qualifier}admitted pressure image with exact alt text"
        ]
    image, target = candidates[0]
    if image.get("alt") != PRESSURE_ALT or target is None:
        return [f"{label} must expose the admitted pressure image with exact alt text"]
    if file_digests.get(target) != pressure_digest:
        return [f"{label}: admitted pressure image has the wrong digest"]
    return []


def _check_home(
    artifact: Path,
    home: object,
    expected_python_version: str,
    file_digests: dict[Path, str],
    pressure_digest: str,
    favicon_digest: str,
    enhanced: bool,
) -> list[str]:
    errors: list[str] = []
    report = errors.append
    release = f"Alpha {expected_python_version}"
    expected = tuple(item.format(release_identity=release) for item in HOME_COPY)
    if not enhanced:
        expected = (
            *expected[:-1],
            expected[-1].replace("repository’s", "repository's"),
        )
    position = 0
    for fragment in expected:
        found = home.visible_text.find(fragment, position)
        if found < 0:
            report(f"/: missing or out-of-order visible text {fragment!r}")
        else:
            position = found + len(fragment)
    if enhanced and RENDERED_SOURCE_SENTENCE not in home.visible_text:
        report(
            "/: curated projection copy must retain the accepted rendered apostrophe"
        )
    start = home.visible_text.find("Featured walkthrough")
    end = home.visible_text.find("Docs", start + 1)
    featured = home.visible_text[start:end].casefold()
    for widening in (
        "flagship",
        "validated flow",
        "production ready",
        "general solver",
        "all backends",
        "benchmark",
        "interactive",
        "run now",
    ):
        if widening in featured:
            report(f"/: featured walkthrough widens its claim with {widening!r}")
    errors.extend(
        _check_pressure_image(
            artifact,
            artifact / "index.html",
            home,
            file_digests,
            pressure_digest,
            "/: featured walkthrough",
        )
    )
    return errors


def _check_case(
    artifact: Path,
    raw: str,
    page: object,
    file_digests: dict[Path, str],
    pressure_digest: str,
    source_sha: str,
    enhanced: bool,
) -> list[str]:
    errors: list[str] = []
    report = errors.append
    markup = _ContentInspection()
    markup.feed(raw)
    markup.close()
    headings = [heading for _, heading in page.headings]
    if enhanced:
        expected_headings = [f"Stage {step} {title}" for _, step, title in STAGES]
        expected_sections = [
            (identifier, step, f"{identifier}-title") for identifier, step, _ in STAGES
        ]
        accessible = [
            markup.accessible_name(label) for _, _, label in expected_sections
        ]
        if markup.stages != expected_sections or accessible != expected_headings:
            report(
                f"Cylinder route must expose six ordered semantic stages: {expected_headings!r}"
            )
    else:
        titles = [title for _, _, title in STAGES]
        if [heading for heading in headings if heading in titles] != titles:
            report("Cylinder route must expose six ordered semantic stages")
    if not any(
        display == "block" and wrapper for display, wrapper in page.math
    ) or not any(display != "block" and not wrapper for display, wrapper in page.math):
        report(
            "Cylinder route must contain distinct block and inline MathML/KaTeX output"
        )
    if markup.katex_mathml == 0 or markup.katex_mathml != markup.katex_html:
        report("Cylinder route must retain both KaTeX HTML and MathML output")
    if enhanced:
        for math in markup.math:
            annotations = math["annotations"]
            if not annotations or (len(annotations) == 1 and not annotations[0]):
                report(
                    "Cylinder route must retain a nonempty TeX annotation for each rendered formula"
                )
                break
            if len(annotations) != 1:
                report(
                    "Cylinder route must retain exactly one TeX annotation for each rendered formula"
                )
                break
    if any(item in page.visible_text for item in ("$$", "\\[", "\\]", "\\(", "\\)")):
        report("Cylinder route exposes raw target math delimiters")
    expected_claim = PUBLIC_CLAIM if enhanced else "one" + PUBLIC_CLAIM[3:]
    if expected_claim not in page.visible_text:
        report("Cylinder route omits the exact bounded public claim")
    if WITNESS_COPY not in page.visible_text:
        report("Cylinder route omits the accepted exact Gmsh CLI 4.15.2 mesh witness")
    source_tokens = (
        (
            "relation momentum continuous on body",
            "2 * dynamic_viscosity * symmetric_part(grad(velocity))",
            "- isotropic_lift(pressure)",
            "relation incompressibility continuous on body",
            "div(velocity) = 0;",
        )
        if enhanced
        else (
            "Eqiora source form",
            "sigma(u,p) = 2 mu sym(grad(u)) - p I",
            "-div(sigma(u,p)) - grad(phi) = 0",
            "div(u) = 0",
        )
    )
    if any(token not in page.visible_text for token in source_tokens):
        report("Cylinder route omits the accepted Eqiora source form")
    errors.extend(
        _check_pressure_image(
            artifact,
            artifact / "gallery/exact-cylinder-steady-stokes/index.html",
            page,
            file_digests,
            pressure_digest,
            "gallery walkthrough",
        )
    )
    admitted_label = ADMITTED_SOURCE_LABEL
    source_base = f"https://github.com/nkiyohara/eqiora/blob/{source_sha}/"
    admitted_href = source_base + ADMITTED_SOURCE_PATH + ADMITTED_SOURCE_FRAGMENT
    sentinels: list[tuple[str, dict[str, str], str, str]] = []
    for tag, attrs, label in page.interactives:
        names = page.id_text
        labelled_ids = attrs.get("aria-labelledby", "").split()
        labelled = " ".join(" ".join(names.get(item, [])) for item in labelled_ids)
        accessible = " ".join(
            (
                labelled or attrs.get("aria-label") or label or attrs.get("title", "")
            ).split()
        )
        handlers = any(name.startswith("on") for name in attrs)
        stage_navigation = tag == "a" and any(
            "eq-stage-marker" in attrs.get("class", "").split()
            and attrs.get("href") == f"#{identifier}"
            and accessible == f"Stage {step} {title}"
            for identifier, step, title in STAGES
        )
        if label == admitted_label or accessible == admitted_label:
            sentinels.append((tag, attrs, label, accessible))
            href = attrs.get("href", "")
            action = (
                tag != "a"
                or attrs.get("role") == "button"
                or handlers
                or not href
                or href.startswith(("#", "javascript:"))
            )
            if action:
                report("Cylinder route navigation link became an action control")
            elif href != admitted_href:
                report(
                    "Cylinder route accepted source-form sentinel must be the exact-head L45-L57 anchor"
                )
            continue
        navigation = stage_navigation or (
            tag == "a" and attrs.get("href", "").startswith(source_base)
        )
        action = not navigation or attrs.get("role") == "button" or handlers
        if action and EXECUTION_CONTROL.search(f"{label} {accessible}"):
            report(
                f"Cylinder route contains an uncontracted execution control {(label, accessible)!r}"
            )
    if len(sentinels) != 1:
        report("Cylinder route must expose one uniquely labelled source-form sentinel")
    if enhanced:
        for phrase in NONCLAIMS:
            if phrase not in page.visible_text:
                report(f"Cylinder claim boundary omits {phrase!r}")
    else:
        legacy = (
            "no arbitrary geometry or provider selection",
            "no 3D, curved, boundary-layer, or adaptive meshing",
            "no mesh/PDE convergence",
            "no drag/lift coefficient, scaled or mesh-independent force, or DFG value",
            "no transient or Navier–Stokes behavior",
            "no vortex shedding",
            "no performance claim",
            "no cross-platform mesh-byte identity or byte-reproducible result",
            "no pixel validation",
            "API presence is neither verification nor maturity",
        )
        folded = page.visible_text.casefold()
        for phrase in legacy:
            if phrase.casefold() not in folded:
                report(f"Cylinder claim boundary omits nonclaim {phrase!r}")
    hrefs = {href for href, _ in page.anchors}
    for relative in (*CASE_SOURCE_PATHS, *CASE_EVIDENCE_PATHS):
        expected = source_base + relative
        if expected not in hrefs:
            report(f"Cylinder route omits exact-head source/evidence link {relative}")
    if PRESSURE_CAPTION not in page.visible_text:
        report("Cylinder route omits the exact admitted caption")
    return errors


def check_starlight_content(
    artifact: Path,
    inspections: dict[Path, tuple[str, object]],
    file_digests: dict[Path, str],
    pressure_digest: str,
    favicon_digest: str,
    source_sha: str,
    expected_python_version: str,
) -> list[str]:
    errors: list[str] = []
    case_value = inspections.get(
        artifact / "gallery/exact-cylinder-steady-stokes/index.html"
    )
    enhanced = bool(case_value and 'class="eq-stage"' in case_value[0])
    home_value = inspections.get(artifact / "index.html")
    if home_value:
        errors += _check_home(
            artifact,
            home_value[1],
            expected_python_version,
            file_digests,
            pressure_digest,
            favicon_digest,
            enhanced,
        )
    if case_value:
        errors += _check_case(
            artifact,
            case_value[0],
            case_value[1],
            file_digests,
            pressure_digest,
            source_sha,
            enhanced,
        )
    reference = inspections.get(artifact / "reference/index.html")
    if reference:
        boundary = (
            REFERENCE_BOUNDARY
            if enhanced
            else "API presence is not verification or maturity."
        )
        required = ("Python", "Rust", "CLI", "control-v2", "MCP", boundary)
        for phrase in required:
            if phrase not in reference[1].visible_text:
                errors.append(f"reference landing omits {phrase!r}")
    home = home_value[1] if home_value else None
    if home:
        anchors = home.anchors
        destinations = (
            ("/get-started/", "Docs"),
            ("/textbooks/", "Textbooks"),
            ("/gallery/", "Gallery"),
            ("/reference/", "Reference"),
            ("/capabilities/", "Capabilities"),
            ("/release-notes/", "Releases"),
            ("https://github.com/nkiyohara/eqiora", "GitHub"),
        )
        positions = []
        for destination in destinations:
            try:
                positions.append(anchors.index(destination))
            except ValueError:
                errors.append(f"public navigation omits {destination!r}")
        if len(positions) == len(destinations) and positions != sorted(positions):
            errors.append("public navigation does not preserve primary then secondary order")
        if ("/evidence/", "Evidence") in anchors:
            errors.append("technical Evidence catalog remains in primary navigation")

    capabilities_value = inspections.get(artifact / "capabilities/index.html")
    textbooks_value = inspections.get(artifact / "textbooks/index.html")
    evidence_value = inspections.get(artifact / "evidence/index.html")
    if capabilities_value:
        capabilities = capabilities_value[1]
        required = (
            "Available",
            "Executable",
            "Checked",
            "Verified",
            "Exact boundary",
            "What this establishes",
            "Current limits",
            "Thermal",
            "Technical catalog",
        )
        for phrase in required:
            if phrase not in capabilities.visible_text:
                errors.append(f"capabilities landing omits {phrase!r}")
    if textbooks_value:
        textbooks = textbooks_value[1]
        for phrase in (
            "Foundations",
            "Physics",
            "Advanced study",
            "1 executable simulation chapter",
        ):
            if phrase not in textbooks.visible_text:
                errors.append(f"textbooks landing omits {phrase!r}")
        for slug, title in TEXTBOOK_SERIES:
            destination = (f"/textbooks/{slug}/", "Open the series map")
            if destination not in textbooks.anchors:
                errors.append(f"textbooks landing omits {title!r} series route")
    for slug, title in TEXTBOOK_SERIES:
        value = inspections.get(artifact / f"textbooks/{slug}/index.html")
        if not value:
            continue
        page = value[1]
        chapter_count = (
            "1 executable simulation chapter"
            if slug == "mathematical-modeling"
            else "0 executable chapters"
        )
        publication_heading = (
            "Publication status"
            if slug == "mathematical-modeling"
            else "Publication boundary"
        )
        for phrase in (title, chapter_count, "Chapter map", publication_heading):
            if phrase not in page.visible_text:
                errors.append(f"textbook {title!r} omits {phrase!r}")
        if slug == "mathematical-modeling":
            for chapter_slug, chapter_title, _ in MODELING_FOUNDATION_CHAPTERS:
                destination = (
                    f"/textbooks/mathematical-modeling/{chapter_slug}/",
                    chapter_title,
                )
                if destination not in page.anchors:
                    errors.append(
                        f"textbook {title!r} omits published chapter {chapter_title!r}"
                    )
    for slug, title, status in MODELING_FOUNDATION_CHAPTERS:
        value = inspections.get(
            artifact / f"textbooks/mathematical-modeling/{slug}/index.html"
        )
        if not value:
            continue
        page = value[1]
        for phrase in (
            title,
            status,
            "Learning outcomes",
            "Deliberate failure",
            "Exercises",
        ):
            if phrase not in page.visible_text:
                errors.append(f"textbook chapter {title!r} omits {phrase!r}")
        if (
            "/textbooks/mathematical-modeling/",
            "Back to the series map",
        ) not in page.anchors:
            errors.append(f"textbook chapter {title!r} omits its series return route")
    if evidence_value:
        evidence = evidence_value[1]
        for phrase in (
            "How to read the technical catalog",
            "Case",
            "Status",
            "Reference",
            "Conformance kit",
            "Target",
            "human-readable Capabilities",
        ):
            if phrase not in evidence.visible_text:
                errors.append(f"technical evidence catalog omits {phrase!r}")
    if capabilities_value and textbooks_value and case_value and evidence_value:
        route_chain = (
            (
                textbooks_value[1],
                "/gallery/exact-cylinder-steady-stokes/",
                "textbooks landing",
            ),
            (
                case_value[1],
                "/capabilities/#exact-cylinder-steady-stokes",
                "Gallery walkthrough",
            ),
            (
                capabilities_value[1],
                "/evidence/#exact-packaged-steady-incompressible-stokes-component",
                "capability summary",
            ),
        )
        for page, href, label in route_chain:
            if href not in {anchor for anchor, _ in page.anchors}:
                errors.append(f"{label} omits the static learning-to-evidence route {href}")
        if "exact-packaged-steady-incompressible-stokes-component" not in evidence_value[1].id_text:
            errors.append("technical evidence route omits the linked exact claim anchor")
    return errors
