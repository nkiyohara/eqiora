import re
from pathlib import Path

__all__ = (
    "PRESSURE_ALT",
    "PRESSURE_CAPTION",
    "CASE_SOURCE_PATHS",
    "CASE_EVIDENCE_PATHS",
    "check_starlight_content",
)

PRESSURE_ALT = "Pressure in pascals for the frozen 2D steady-Stokes exact-cylinder demonstration, shown with a viridis color scale and the 104-triangle affine mesh overlaid. Presentation image only; linked Result evidence carries the numerical claim."
PRESSURE_CAPTION = "Pressure (Pa), frozen exact-cylinder steady-Stokes demonstration at c6b7a21f52ae1acf941d26319d2499ed89152c15; presentation only, not validation."
PUBLIC_CLAIM = "One frozen 2D steady incompressible Stokes exact-cylinder demonstration, rendered from its accepted public Result path and linked evidence."
RENDERED_SOURCE_SENTENCE = "This website is a curated projection, not a parallel specification. Detailed contracts remain in the repository’s architecture, RFCs, capability matrix, and validated verify manifests."
REFERENCE_BOUNDARY = "API presence is neither capability evidence nor maturity."
EVIDENCE_LABELS = {"Result evidence", "Pressure-still presentation case"}
STAGES = (
    ("problem-setup", "1", "Problem setup"),
    ("model-definition", "2", "Eqiora model definition"),
    ("mesh-and-boundaries", "3", "Mesh and boundaries"),
    ("submit-and-result", "4", "Submit and result"),
    ("pressure-visualization", "5", "Pressure visualization"),
    ("verified-boundary", "6", "Verified and not claimed"),
)
NONCLAIMS = (
    "No curved elements.",
    "No mesh/PDE convergence.",
    "No drag/lift coefficient, scaled or mesh-independent force, or DFG value.",
    "No transient or Navier–Stokes behavior.",
    "No vortex shedding.",
    "No 3D.",
    "No production mesher.",
    "No performance claim.",
    "No cross-platform/byte-reproducible result.",
    "No pixel validation.",
    "All 104 mesh vertices lie on a boundary",
    "103 velocity vertices are essential",
    "the only free velocity vertex is the outlet midpoint",
    "API presence is neither verification nor maturity.",
)
CASE_SOURCE_PATHS = (
    "examples/python/exact_cylinder_stokes_marimo.py",
    "examples/python/exact_cylinder_stokes.py",
    "examples/python/exact_cylinder_geometry.py",
    "examples/python/exact_cylinder_mesh.py",
    "verify/fluid/packaged-steady-stokes-2d/models/direct.eqi",
    "verify/fluid/packaged-steady-stokes-2d/package-v0.1.0/src/incompressible.eqi",
    "packages/Eqiora.Fluid.Incompressible/src/incompressible.eqi",
)
CASE_EVIDENCE_PATHS = (
    "verify/artifacts/current-model-canonical-identity/README.md",
    "verify/fluid/packaged-steady-stokes-2d/README.md",
    "verify/fluid/exact-circular-hole-stokes-2d/README.md",
    "verify/geometry/exact-circular-hole-geometry/README.md",
    "verify/geometry/circular-hole-chordal-realization-binding/README.md",
    "verify/geometry/circular-hole-chordal-reference-mesh/README.md",
    "verify/interfaces/python-exact-circular-hole-geometry/README.md",
    "verify/interfaces/python-circular-hole-chordal-mesh/README.md",
    "verify/interfaces/python-exact-cylinder-stokes-result/README.md",
    "verify/interfaces/python-exact-cylinder-pressure-still/README.md",
    "verify/interfaces/python-exact-cylinder-stokes-marimo/README.md",
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
    "Reference",
    "Browse exact-commit Python, Rust, CLI, control-v2, and MCP surfaces. API presence is not verification or maturity.",
    "Evidence",
    "Inspect the generated capability-to-case index and the manifests that own each bounded claim.",
    "{release_identity}",
    "Eqiora is alpha research software under active development. The capability matrix and generated evidence catalog bound what is currently supported; this site does not widen those claims.",
    "One source of truth",
    RENDERED_SOURCE_SENTENCE,
)
EXECUTION_CONTROL = re.compile(
    r"\b(?:run|submit|reset|start|begin|try|solv\w*|execut\w*|simulat\w*|comput\w*|calculat\w*|launch\w*|evaluat\w*|process\w*|generat\w*|analy[sz]\w*|predict\w*)\b",
    re.IGNORECASE,
)


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
    pressure = [
        image
        for image in home.images
        if image.get("src") == "/assets/pressure.png"
        and file_digests.get(artifact / "assets/pressure.png") == pressure_digest
        and image.get("alt") == PRESSURE_ALT
    ]
    if len(pressure) != 1:
        report(
            "/: featured walkthrough must expose the admitted pressure image with exact alt text"
        )
    brand = [
        image
        for image in home.images
        if image.get("src") == "/assets/brand.svg"
        and file_digests.get(artifact / "assets/brand.svg") == favicon_digest
        and image.get("_ancestor_href") == "/"
    ]
    if not brand:
        report("/: header does not link the exact Eqiora mark home")
    if ("/", "Eqiora") not in home.anchors:
        report("/: header does not expose the visible Eqiora home link")
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
    headings = [heading for _, heading in page.headings]
    if enhanced:
        expected_headings = [f"Stage {step} {title}" for _, step, title in STAGES]
        observed = [heading for heading in headings if heading.startswith("Stage ")]
        sections = re.findall(
            r'<section class="eq-stage" id="([^"]+)" data-step="([^"]+)" aria-labelledby="([^"]+)">',
            raw,
        )
        expected_sections = [
            (identifier, step, f"{identifier}-title") for identifier, step, _ in STAGES
        ]
        if observed != expected_headings or sections != expected_sections:
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
    if "katex-mathml" not in raw or "katex-html" not in raw:
        report("Cylinder route must retain both KaTeX HTML and MathML output")
    annotations = re.findall(
        r'<annotation\s+encoding="application/x-tex">([^<]+)</annotation>', raw
    )
    if enhanced and (
        len(annotations) != 2 or any(not item.strip() for item in annotations)
    ):
        report(
            "Cylinder route must retain a nonempty TeX annotation for each rendered formula"
        )
    if any(item in page.visible_text for item in ("$$", "\\[", "\\]", "\\(", "\\)")):
        report("Cylinder route exposes raw target math delimiters")
    expected_claim = PUBLIC_CLAIM if enhanced else "one" + PUBLIC_CLAIM[3:]
    if expected_claim not in page.visible_text:
        report("Cylinder route omits the exact bounded public claim")
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
    pressure = [
        image for image in page.images if image.get("src") == "/assets/pressure.png"
    ]
    if (
        len(pressure) != 1
        or pressure[0].get("alt") != PRESSURE_ALT
        or file_digests.get(artifact / "assets/pressure.png") != pressure_digest
    ):
        report(
            "Cylinder route must expose the admitted pressure bytes once with exact alt text"
        )
    admitted_label = "Eqiora source form: canonical intent/submit/result cells"
    source_base = f"https://github.com/nkiyohara/eqiora/blob/{source_sha}/"
    admitted_href = source_base + CASE_SOURCE_PATHS[0]
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
        if label == admitted_label or accessible == admitted_label:
            if (
                tag != "a"
                or attrs.get("href") != admitted_href
                or attrs.get("role") == "button"
                or handlers
            ):
                report("Cylinder route navigation link became an action control")
            continue
        navigation = tag == "a" and attrs.get("href", "").startswith(source_base)
        action = not navigation or attrs.get("role") == "button" or handlers
        if action and EXECUTION_CONTROL.search(f"{label} {accessible}"):
            report(
                f"Cylinder route contains an uncontracted execution control {(label, accessible)!r}"
            )
    if enhanced:
        for phrase in NONCLAIMS:
            if phrase not in page.visible_text:
                report(f"Cylinder claim boundary omits {phrase!r}")
    else:
        legacy = (
            "no curved elements",
            "no mesh/PDE convergence",
            "no drag/lift coefficient, scaled or mesh-independent force, or DFG value",
            "no transient or Navier–Stokes behavior",
            "no vortex shedding",
            "no 3D",
            "no production mesher",
            "no performance claim",
            "no cross-platform byte reproducibility",
            "pixels are not validation",
            "all 104 vertices are on the boundary",
            "only the outlet midpoint velocity vertex is free",
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
    labels = {label for _, label in page.anchors}
    if PRESSURE_CAPTION not in page.visible_text or not EVIDENCE_LABELS <= labels:
        report(
            "Cylinder route omits the exact admitted caption or its two visible evidence links"
        )
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
    return errors
