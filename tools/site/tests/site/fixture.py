from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

SOURCE_SHA = "a" * 40
REPOSITORY = Path(__file__).resolve().parents[4]


def _load_checker():
    path = REPOSITORY / "tools/site/check_site.py"
    spec = importlib.util.spec_from_file_location("alpha2_site_check", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


checker = _load_checker()


def _write(path: Path, value: str | bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(value, bytes):
        path.write_bytes(value)
    else:
        path.write_text(value, encoding="utf-8")


def _workflow() -> str:
    paths = "\n".join(
        f'      - "{path}"' for path in sorted(checker.REQUIRED_TRIGGER_PATTERNS)
    )
    return f"""name: fixture
on:
  pull_request:
    paths:
{paths}
  push:
    branches:
      - main
    paths:
{paths}
jobs:
  build:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@{"1" * 40}
      - run: |
          test "$(git rev-parse HEAD)" = "$GITHUB_SHA"
          scratch="$RUNNER_TEMP/eqiora-site.fixture"
          mkdir -p "$scratch/source"
          git archive --format=tar "$GITHUB_SHA" | tar -xf - -C "$scratch/source"
          echo "EQIORA_SITE_SOURCE_ROOT=$scratch/source"
          echo eqiora-pw-1.62.1-r1234
          npx playwright install --with-deps --only-shell chromium
          echo 'HeadlessChrome 151.0.7922.34'
          unshare --net bash -c 'ip link set lo up; setpriv true'
          export npm_config_offline=true
          export CARGO_NET_OFFLINE=true
          export UV_OFFLINE=1
"""


def _runner() -> str:
    return """#!/usr/bin/env bash
export npm_config_offline=true CARGO_NET_OFFLINE=true UV_OFFLINE=1
export EQIORA_SITE_CARGO_VERSION=0.1.0-alpha.1 EQIORA_SITE_PYTHON_VERSION=0.1.0a1
python3 -m unittest tools.site.tests.test_site_tools -v
python3 tools/docs/generate_interface_reference.py --repository . --eqiora-binary bin/eqiora --mcp-binary bin/eqiora-mcp --check
python3 tools/site/build_rust_reference.py --rustdoc-root rustdoc/doc --output rustdoc-stage
python3 tools/site/generate_evidence_catalog.py
python3 tools/site/check_site.py check
python3 tools/site/check_site.py serve
"""


def _package_files(root: Path) -> None:
    dependencies = checker.RUNTIME_PINS
    dev_dependencies = checker.DEVELOPMENT_PINS
    package = {
        "engines": {"node": "24.18.1", "npm": "11.16.0"},
        "dependencies": dependencies,
        "devDependencies": dev_dependencies,
    }
    lock_packages = {
        "": {
            "dependencies": dependencies,
            "devDependencies": dev_dependencies,
            "engines": package["engines"],
        }
    }
    lock_packages.update(
        {
            f"node_modules/{name}": {
                "version": version,
                "integrity": "sha512-fixture",
            }
            for name, version in checker.DIRECT_PINS.items()
        }
    )
    lock = {"packages": lock_packages}
    _write(root / "docs/site/package.json", json.dumps(package))
    _write(root / "docs/site/package-lock.json", json.dumps(lock))


def _head(route: str) -> str:
    return f"""<head>
<meta charset="utf-8">
<meta property="og:image" content="https://eqiora.org/social-card.svg">
<link rel="canonical" href="https://eqiora.org{route}">
<link rel="icon" href="/favicon.svg">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<link rel="stylesheet" href="/assets/site.css">
</head>"""


def _nav() -> str:
    return """<nav aria-label="Primary">
<a href="/"><img src="/assets/brand.svg" alt="">Eqiora</a>
<a href="/get-started/">Docs</a>
<a href="/gallery/">Gallery</a>
<a href="/reference/">Reference</a>
<a href="/evidence/">Evidence</a>
<a href="https://github.com/nkiyohara/eqiora">GitHub</a>
</nav>"""


def _page(route: str, body: str) -> str:
    return f"<!doctype html><html>{_head(route)}<body><header>{_nav()}</header><main>{body}</main></body></html>"


def _exact_links() -> str:
    links = []
    for relative in (*checker.CASE_SOURCE_PATHS, *checker.CASE_EVIDENCE_PATHS):
        url = f"https://github.com/nkiyohara/eqiora/blob/{SOURCE_SHA}/{relative}"
        label = Path(relative).parent.name + " " + Path(relative).name
        links.append(f'<a href="{url}">{label}</a>')
    return "".join(links)


def _case_body() -> str:
    nonclaims = (
        "no curved elements; no mesh/PDE convergence; no drag/lift coefficient, "
        "scaled or mesh-independent force, or DFG value; no transient or "
        "Navier–Stokes behavior; no vortex shedding; no 3D; no production mesher; "
        "no performance claim; no cross-platform byte reproducibility; pixels are "
        "not validation. All 104 vertices are on the boundary and only the outlet "
        "midpoint velocity vertex is free. API presence is neither verification "
        "nor maturity."
    )
    return f"""<h1>Exact-cylinder steady Stokes</h1>
<p>Static walkthrough · canonical Marimo source available</p>
<p>one frozen 2D steady incompressible Stokes exact-cylinder demonstration, rendered from its accepted public Result path and linked evidence.</p>
<section><h2>Problem setup</h2><p>2.2m x 0.41m channel; centre [0.2,0.2]m; radius 0.05m.</p>
<span class="katex"><span class="katex-mathml"><math><mi>H</mi></math></span><span class="katex-html">H</span></span></section>
<section><h2>Eqiora model definition</h2>
<div class="eqiora-math-region" role="region" aria-label="Displayed equation" tabindex="0"><span class="katex-display"><span class="katex"><span class="katex-mathml"><math display="block"><mi>sigma</mi></math></span><span class="katex-html">sigma</span></span></span></div>
<p>Eqiora source form</p><pre>sigma(u,p) = 2 mu sym(grad(u)) - p I
-div(sigma(u,p)) - grad(phi) = 0
div(u) = 0</pre></section>
<section><h2>Mesh and boundaries</h2><p>50-chord and 104-triangle affine demonstration mesh; coarse-mesh warning.</p></section>
<section><h2>Submit and result</h2><p>One immutable SteadyStokes intent, resolve, submit, and Result carrier.</p></section>
<section><h2>Pressure visualization</h2><figure><img src="/assets/pressure.png" alt="{checker.PRESSURE_ALT}"><figcaption>{checker.PRESSURE_CAPTION} <a href="https://github.com/nkiyohara/eqiora/blob/{SOURCE_SHA}/verify/interfaces/python-exact-cylinder-stokes-result/README.md">Result evidence</a> <a href="https://github.com/nkiyohara/eqiora/blob/{SOURCE_SHA}/verify/interfaces/python-exact-cylinder-pressure-still/README.md">Pressure-still presentation case</a></figcaption></figure><p>Presentation, not evidence.</p></section>
<section><h2>Verified and not claimed</h2><p>{nonclaims}</p>{_exact_links()}</section>"""


def _home_body() -> str:
    return f"""<h1>Model meaning once. Realize it many ways.</h1>
<p>Eqiora is an open-source, meaning-first foundation for scientific modeling, simulation, differentiation, and execution.</p>
<p>Its central boundary is simple:</p>
<p>A model states typed mathematical relations. A realization chooses how those relations are discretized, solved, and executed.</p>
<p>That separation lets block diagrams, acausal physical networks, PDE fields, hybrid dynamics, and reusable components share one canonical meaning without making a numerical method or hardware backend part of the model.</p>
<a href="/get-started/">Get started</a><a href="/gallery/">Explore gallery</a>
<article><p>Featured walkthrough</p><h2>Exact-cylinder steady Stokes</h2><img src="/assets/pressure.png" alt="{checker.PRESSURE_ALT}"><p>Follow one frozen 2D steady-Stokes problem from model definition and named boundaries through one submit/Result path to an independently admitted static pressure image.</p><p>Python</p><p>2D</p><p>steady Stokes</p><a href="/gallery/exact-cylinder-steady-stokes/">View the static walkthrough</a></article>
<article><h2>Docs</h2><p>Learn the Model–Realization boundary and start from bounded examples.</p></article>
<article><h2>Reference</h2><p>Browse exact-commit Python, Rust, CLI, control-v2, and MCP surfaces. API presence is not verification or maturity.</p></article>
<article><h2>Evidence</h2><p>Inspect the generated capability-to-case index and the manifests that own each bounded claim.</p></article>
<p>Alpha {{python_version}}</p><p>Eqiora is alpha research software under active development. The capability matrix and generated evidence catalog bound what is currently supported; this site does not widen those claims.</p>
<h2>One source of truth</h2><p>This website is a curated projection, not a parallel specification. Detailed contracts remain in the repository's architecture, RFCs, capability matrix, and validated verify manifests.</p>"""


def _artifact(root: Path, blobs: dict[str, bytes], python_version: str) -> Path:
    artifact = root / "artifact"
    _write(artifact / "social-card.svg", blobs["social"])
    _write(artifact / "favicon.svg", blobs["favicon"])
    _write(artifact / "apple-touch-icon.png", blobs["apple"])
    _write(artifact / "assets/pressure.png", blobs["pressure"])
    _write(artifact / "assets/brand.svg", blobs["favicon"])
    _write(artifact / "assets/KaTeX_Main-Regular.woff2", b"font")
    _write(
        artifact / "assets/site.css",
        "@font-face{font-family:KaTeX;src:url('./KaTeX_Main-Regular.woff2')}\n",
    )
    _write(
        artifact / "pagefind/pagefind.js",
        "export async function search(){return {results:[]}}\n",
    )

    pages = {
        "/": _home_body().format(python_version=python_version),
        "/gallery/": '<h1>Gallery</h1><a href="/gallery/exact-cylinder-steady-stokes/">Exact-cylinder steady Stokes</a>',
        "/gallery/exact-cylinder-steady-stokes/": _case_body(),
        "/reference/": '<h1>Reference</h1><p>Python Rust CLI control-v2 MCP</p><p>API presence is not verification or maturity.</p><form action="/reference/"><input aria-label="Search"></form>',
        "/reference/python/eqiora/": "<h1>eqiora Python module</h1><p>Diagnostic</p>",
        "/reference/rust/": '<h1>Rust reference</h1><p>eqiora::Diagnostic stable eqiora::api::CadBoxIntentV1 transitional eqiora::api module</p><a href="/reference/rust/api/eqiora/struct.Diagnostic.html">Diagnostic</a>',
        "/reference/cli/": "<h1>CLI</h1><p>eqiora check</p>",
        "/reference/control-v2/": "<h1>control-v2</h1><p>eqiora.control/v2</p>",
        "/reference/mcp/": "<h1>MCP</h1><p>eqiora.model.compile_check</p>",
        "/examples/": '<h1>Examples</h1><a href="/gallery/">Gallery</a>',
        "/404.html": "<h1>Page not found</h1>",
    }
    for route, body in pages.items():
        relative = checker.ROUTES[route]
        _write(artifact / relative, _page(route, body))
    _write(
        artifact / "reference/rust/api/eqiora/struct.Diagnostic.html",
        '<!doctype html><html><body><main><h1>Struct Diagnostic</h1><a href="index.html">eqiora</a></main></body></html>',
    )
    _write(
        artifact / "reference/rust/api/eqiora/index.html",
        '<!doctype html><html><body><main><h1>Crate eqiora</h1><a href="struct.Diagnostic.html">Diagnostic</a></main></body></html>',
    )
    _write(
        artifact / "get-started/index.html",
        _page("/get-started/", "<h1>Get started</h1>"),
    )
    _write(artifact / "evidence/index.html", _page("/evidence/", "<h1>Evidence</h1>"))
    _write(
        artifact / "capabilities/index.html",
        _page("/capabilities/", "<h1>Capabilities</h1>"),
    )
    urls = "".join(
        f"<url><loc>https://eqiora.org{route}</loc></url>"
        for route in checker.SITEMAP_ROUTES
    )
    _write(
        artifact / "sitemap-index.xml", f'<?xml version="1.0"?><urlset>{urls}</urlset>'
    )
    _write(
        artifact / "robots.txt",
        "User-agent: *\nAllow: /\nSitemap: https://eqiora.org/sitemap-index.xml\n",
    )
    return artifact


def make_fixture(root: Path, cargo_version: str = "0.1.0-alpha.1"):
    python_version = cargo_version.replace("-alpha.", "a")
    blobs = {
        "pressure": b"fixture admitted pressure",
        "publication": b'{"fixture":"publication"}\n',
        "social": b"<svg><title>Eqiora</title></svg>\n",
        "favicon": b"<svg><title>Eqiora mark</title></svg>\n",
        "apple": b"fixture apple png",
    }
    _package_files(root)
    _write(
        root / "Cargo.toml",
        f'[workspace]\nmembers = []\n[workspace.package]\nversion = "{cargo_version}"\n',
    )
    _write(
        root / "tools/release/python_candidate_common.py",
        "def python_distribution_version(version):\n"
        "    return version.replace('-alpha.', 'a')\n",
    )
    _write(root / ".github/workflows/pages.yml", _workflow())
    _write(root / "tools/site/run_offline_site_checks.sh", _runner())
    _write(
        root / "docs/site/src/components/site/ExactSourceLink.astro",
        "---\nconst source = import.meta.env.EQIORA_SITE_SOURCE_SHA;\n"
        "const kinds = ['blob', 'tree'];\n---\n<a href={source}>{Astro.slots}</a>\n",
    )
    _write(
        root / "docs/site/src/components/site/ReleaseIdentity.astro",
        "---\nconst cargo = import.meta.env.EQIORA_SITE_CARGO_VERSION;\n"
        "const python = import.meta.env.EQIORA_SITE_PYTHON_VERSION;\n---\n"
        "<span>{cargo}{python}</span>\n",
    )
    _write(
        root / "docs/site/src/content/docs/index.mdx",
        "import ExactSourceLink from '@components/site/ExactSourceLink.astro';\n"
        "import ReleaseIdentity from '@components/site/ReleaseIdentity.astro';\n"
        '<ExactSourceLink path="AGENTS.md" kind="blob">AGENTS</ExactSourceLink>\n'
        "<ReleaseIdentity />\n",
    )
    _write(
        root / "docs/site/src/content/docs/evidence/index.mdx",
        "import ExactSourceLink from '@components/site/ExactSourceLink.astro';\n"
        '<ExactSourceLink path="verify/example/case.toml" kind="blob">case</ExactSourceLink>\n',
    )
    _write(
        root / "docs/site/astro.config.mjs",
        "const required = [\n"
        "  'src/components/site/ExactSourceLink.astro',\n"
        "  'src/components/site/ReleaseIdentity.astro',\n"
        "];\n",
    )
    _write(
        root / "docs/site/src/assets/gallery/exact-cylinder-pressure.png",
        blobs["pressure"],
    )
    _write(
        root
        / "docs/site/src/data/gallery/exact-cylinder-steady-stokes.publication.json",
        blobs["publication"],
    )
    _write(root / "docs/site/public/social-card.svg", blobs["social"])
    _write(root / "docs/site/public/favicon.svg", blobs["favicon"])
    _write(root / "docs/site/public/apple-touch-icon.png", blobs["apple"])
    identities = checker.SiteIdentities(
        pressure=checker.sha256(
            root / "docs/site/src/assets/gallery/exact-cylinder-pressure.png"
        ),
        publication=checker.sha256(
            root
            / "docs/site/src/data/gallery/exact-cylinder-steady-stokes.publication.json"
        ),
        social=checker.sha256(root / "docs/site/public/social-card.svg"),
        favicon=checker.sha256(root / "docs/site/public/favicon.svg"),
        apple_touch=checker.sha256(root / "docs/site/public/apple-touch-icon.png"),
    )
    artifact = _artifact(root, blobs, python_version)
    return artifact, identities
