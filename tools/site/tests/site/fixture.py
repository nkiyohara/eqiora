from __future__ import annotations

import importlib.util
import json
import os
import re
import selectors
import signal
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Sequence

SOURCE_SHA = "a" * 40
REPOSITORY = Path(__file__).resolve().parents[4]
PRESSURE_ALT = (
    "Pressure in pascals for a 2D steady-Stokes exact-cylinder demonstration, "
    "shown with a viridis color scale and its current Gmsh mesh overlaid. "
    "Presentation image only; no numerical or mesh-output oracle."
)
PUBLIC_CLAIM = (
    "One presentation-only 2D steady incompressible Stokes exact-cylinder "
    "demonstration rendered through exact Geometry, typed Gmsh policy, and the "
    "root Result path; output counts, digests, numerical values, and pixels are "
    "not independently verified."
)
WITNESS_COPY = (
    "The current Gmsh output is presentation input, not a fixed mesh or scientific oracle."
)
CASE_EVIDENCE_PATHS = (
    "verify/artifacts/current-model-canonical-identity/README.md",
    "verify/fluid/packaged-steady-stokes-2d/README.md",
    "verify/geometry/exact-circular-hole-geometry/README.md",
    "verify/interfaces/python-exact-circular-hole-geometry/README.md",
)
NONCLAIMS = (
    "No arbitrary geometry or provider selection.",
    "No 3D, curved, boundary-layer, or adaptive meshing.",
    "No mesh/PDE convergence.",
    "No drag/lift coefficient, scaled or mesh-independent force, or DFG value.",
    "No transient or Navier–Stokes behavior.",
    "No vortex shedding.",
    "No performance claim.",
    "No cross-platform mesh-byte identity or byte-reproducible Result.",
    "No pixel validation.",
    "API presence is neither verification nor maturity.",
)
SITE_ROUTES = (
    "/",
    "/api/",
    "/architecture/",
    "/capabilities/",
    "/concepts/",
    "/contributing/",
    "/evidence/",
    "/examples/",
    "/gallery/",
    "/gallery/exact-cylinder-steady-stokes/",
    "/gallery/mixed-boundary-elasticity/",
    "/get-started/",
    "/python/",
    "/python/differentiation/",
    "/python/execution-and-arrays/",
    "/python/modeling/",
    "/reference/",
    "/reference/cli/",
    "/reference/control-v2/",
    "/reference/mcp/",
    "/reference/python/",
    "/reference/python/diff/",
    "/reference/python/eqiora/",
    "/reference/python/fluid/",
    "/reference/python/fsi/",
    "/reference/python/geometry/",
    "/reference/python/jax/",
    "/reference/python/matplotlib/",
    "/reference/python/meshing/",
    "/reference/python/solid/",
    "/reference/python/torch/",
    "/reference/python/trajectory/",
    "/reference/rust/",
    "/release-notes/",
    "/textbooks/",
    "/textbooks/circuits-dynamics-hybrid/",
    "/textbooks/fluid-mechanics-cfd/",
    "/textbooks/heat-mass-transfer/",
    "/textbooks/mathematical-modeling/",
    "/textbooks/mathematical-modeling/models-not-simulations/",
    "/textbooks/mathematical-modeling/quantities-dimensions-units/",
    "/textbooks/numerical-simulation/",
    "/textbooks/structural-mechanics-fem/",
    "/404.html",
)
GIT_OBJECT_REPOSITORY_VARIABLE = "EQIORA_SITE_GIT_OBJECT_REPOSITORY"
SOURCE_SHA_VARIABLE = "EQIORA_SITE_SOURCE_SHA"
GIT_TIMEOUT_SECONDS = 30
GIT_IDENTITY_OUTPUT_LIMIT = 65_536
GIT_OBJECT_OUTPUT_LIMIT = 34_088_961
GIT_STDERR_LIMIT = 65_536
_LOWER_OBJECT_ID = re.compile(rb"[0-9a-f]{40}\n")
_GIT_EXECUTABLE = Path(shutil.which("git", path=os.defpath) or "/usr/bin/git").resolve()
_GIT_ENVIRONMENT = {
    "LC_ALL": "C",
    "LANG": "C",
    "TZ": "UTC",
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_OPTIONAL_LOCKS": "0",
    "GIT_TERMINAL_PROMPT": "0",
    "GIT_NO_REPLACE_OBJECTS": "1",
}
_HISTORICAL_COMMITS = {
    "5d2a9bef58c2df32cd6b14c5b6dd876beac7144f",
    "68c9c2fe245ac52cc20dcf5a65a2455de507f0dc",
    "19968da984c16e718baeb9faa5aae04260896c29",
}
_HISTORICAL_OBJECTS = {
    "3237f739098498ac46bfdd6a993c00b0575900f3",
    "57f8b9b476c04b8103b5a43c8a30504c0e2fa1fb",
    "47dc3e3d863cfb5727b87d785d09abf9743c0a72",
    "61c1bbede492aef4a9c85fa364d031e012621809",
    "20701fe8909295b980c1da7cf3eab366f8d5f27c",
    "6e685495bf6989e1ad902a7e88c199557285cbee",
    "1d19473c487b8035608cc88cbd99757f2b95865a",
    "21d5f0bc5213bca02336040f1085c7d52c63588f",
}
_HISTORICAL_QUERIES = {
    ("rev-parse", "--verify", "HEAD^{commit}"): GIT_IDENTITY_OUTPUT_LIMIT,
    ("rev-parse", "--verify", "HEAD^{tree}"): GIT_IDENTITY_OUTPUT_LIMIT,
    (
        "rev-parse",
        "5d2a9bef58c2df32cd6b14c5b6dd876beac7144f^{tree}",
    ): GIT_IDENTITY_OUTPUT_LIMIT,
    (
        "rev-parse",
        "5d2a9bef58c2df32cd6b14c5b6dd876beac7144f:.github/workflows/pages.yml",
    ): GIT_IDENTITY_OUTPUT_LIMIT,
    (
        "rev-parse",
        "5d2a9bef58c2df32cd6b14c5b6dd876beac7144f:CLAUDE.md",
    ): GIT_IDENTITY_OUTPUT_LIMIT,
    (
        "rev-parse",
        "5d2a9bef58c2df32cd6b14c5b6dd876beac7144f:AGENTS.md",
    ): GIT_IDENTITY_OUTPUT_LIMIT,
    (
        "ls-tree",
        "5d2a9bef58c2df32cd6b14c5b6dd876beac7144f",
        "--",
        "AGENTS.md",
        "CLAUDE.md",
    ): GIT_IDENTITY_OUTPUT_LIMIT,
    (
        "rev-parse",
        "68c9c2fe245ac52cc20dcf5a65a2455de507f0dc^{tree}",
    ): GIT_IDENTITY_OUTPUT_LIMIT,
    (
        "rev-parse",
        "68c9c2fe245ac52cc20dcf5a65a2455de507f0dc:.github/workflows/pages.yml",
    ): GIT_IDENTITY_OUTPUT_LIMIT,
    (
        "rev-parse",
        "19968da984c16e718baeb9faa5aae04260896c29^{tree}",
    ): GIT_IDENTITY_OUTPUT_LIMIT,
    (
        "archive",
        "--format=tar",
        "19968da984c16e718baeb9faa5aae04260896c29",
    ): GIT_OBJECT_OUTPUT_LIMIT,
    (
        "ls-tree",
        "-r",
        "-z",
        "19968da984c16e718baeb9faa5aae04260896c29",
    ): GIT_OBJECT_OUTPUT_LIMIT,
    (
        "show",
        "19968da984c16e718baeb9faa5aae04260896c29:docs/site/package-lock.json",
    ): GIT_OBJECT_OUTPUT_LIMIT,
    (
        "show",
        "19968da984c16e718baeb9faa5aae04260896c29:CLAUDE.md",
    ): GIT_IDENTITY_OUTPUT_LIMIT,
    (
        "show",
        "19968da984c16e718baeb9faa5aae04260896c29:AGENTS.md",
    ): GIT_IDENTITY_OUTPUT_LIMIT,
}
_HISTORICAL_QUERIES.update(
    {
        ("cat-file", "-e", f"{commit}^{{commit}}"): GIT_IDENTITY_OUTPUT_LIMIT
        for commit in _HISTORICAL_COMMITS
    }
)
_HISTORICAL_QUERIES.update(
    {
        ("cat-file", "-e", object_id): GIT_IDENTITY_OUTPUT_LIMIT
        for object_id in _HISTORICAL_OBJECTS
    }
)
_HISTORICAL_QUERIES.update(
    {
        ("cat-file", "blob", object_id): GIT_IDENTITY_OUTPUT_LIMIT
        for object_id in {
            "57f8b9b476c04b8103b5a43c8a30504c0e2fa1fb",
            "47dc3e3d863cfb5727b87d785d09abf9743c0a72",
            "61c1bbede492aef4a9c85fa364d031e012621809",
            "6e685495bf6989e1ad902a7e88c199557285cbee",
        }
    }
)


class GitObjectAuthorityError(AssertionError):
    pass


@dataclass(frozen=True)
class GitObjectAuthority:
    root: Path
    head: str
    tree: str


def git_read_environment() -> dict[str, str]:
    return {**_GIT_ENVIRONMENT, "PATH": os.defpath}


def _bounded_command(
    command: Sequence[str],
    *,
    output_limit: int,
    executable: Path = _GIT_EXECUTABLE,
) -> bytes:
    process = subprocess.Popen(
        [str(executable), *command],
        env=_GIT_ENVIRONMENT,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    assert process.stdout is not None and process.stderr is not None
    streams = selectors.DefaultSelector()
    streams.register(process.stdout, selectors.EVENT_READ, ("stdout", output_limit))
    streams.register(process.stderr, selectors.EVENT_READ, ("stderr", GIT_STDERR_LIMIT))
    captured = {"stdout": bytearray(), "stderr": bytearray()}
    deadline = time.monotonic() + GIT_TIMEOUT_SECONDS
    try:
        while streams.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise GitObjectAuthorityError("Git object authority command timed out")
            ready = streams.select(remaining)
            if not ready:
                raise GitObjectAuthorityError("Git object authority command timed out")
            for key, _ in ready:
                label, limit = key.data
                chunk = os.read(key.fd, min(65_536, limit + 1 - len(captured[label])))
                if not chunk:
                    streams.unregister(key.fileobj)
                    continue
                captured[label].extend(chunk)
                if len(captured[label]) > limit:
                    raise GitObjectAuthorityError(
                        f"Git object authority {label} exceeded its bound"
                    )
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise GitObjectAuthorityError("Git object authority command timed out")
        returncode = process.wait(timeout=remaining)
    except (GitObjectAuthorityError, subprocess.TimeoutExpired):
        if process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        process.wait()
        raise GitObjectAuthorityError(
            "Git object authority command failed or timed out"
        )
    finally:
        streams.close()
        process.stdout.close()
        process.stderr.close()
    stdout, stderr = bytes(captured["stdout"]), bytes(captured["stderr"])
    if returncode != 0 or stderr:
        raise GitObjectAuthorityError(
            "Git object authority command failed or emitted stderr"
        )
    return stdout


def _git_read(
    root: Path,
    arguments: Sequence[str],
    *,
    output_limit: int = GIT_IDENTITY_OUTPUT_LIMIT,
    executable: Path = _GIT_EXECUTABLE,
) -> bytes:
    return _bounded_command(
        [
            "--no-replace-objects",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
            "-C",
            str(root),
            *arguments,
        ],
        output_limit=output_limit,
        executable=executable,
    )


def _canonical_object_id(output: bytes, label: str) -> str:
    if _LOWER_OBJECT_ID.fullmatch(output) is None:
        raise GitObjectAuthorityError(
            f"Git object authority returned malformed {label} identity"
        )
    return output[:-1].decode("ascii")


def git_object_authority(
    *,
    repository: Path = REPOSITORY,
    environment: Mapping[str, str] | None = None,
    executable: Path = _GIT_EXECUTABLE,
) -> GitObjectAuthority:
    values = os.environ if environment is None else environment
    supplied = GIT_OBJECT_REPOSITORY_VARIABLE in values
    raw = values.get(GIT_OBJECT_REPOSITORY_VARIABLE, "")
    repository = repository.resolve(strict=True)
    if supplied:
        if not raw:
            raise GitObjectAuthorityError("Git object authority is empty")
        candidate = Path(raw)
        if not candidate.is_absolute():
            raise GitObjectAuthorityError("Git object authority is not absolute")
        try:
            root = candidate.resolve(strict=True)
        except OSError as error:
            raise GitObjectAuthorityError(
                "Git object authority is unavailable"
            ) from error
        if str(candidate) != str(root):
            raise GitObjectAuthorityError("Git object authority is not canonical")
        if not root.is_dir() or root.is_symlink():
            raise GitObjectAuthorityError(
                "Git object authority is not a real non-symlink directory"
            )
        if os.path.lexists(repository / ".git"):
            raise GitObjectAuthorityError("direct archive unexpectedly contains .git")
        if root == repository or root.is_relative_to(repository):
            raise GitObjectAuthorityError("Git object authority overlaps the archive")
    else:
        if not os.path.lexists(repository / ".git"):
            raise GitObjectAuthorityError(
                "a .git-absent archive requires EQIORA_SITE_GIT_OBJECT_REPOSITORY"
            )
        root = repository

    try:
        path_limit = os.pathconf(root, "PC_PATH_MAX")
    except (OSError, ValueError):
        path_limit = 4096
    if len(os.fsencode(root)) >= path_limit:
        raise GitObjectAuthorityError("Git object authority exceeds the path bound")

    top_level_output = _git_read(
        root, ["rev-parse", "--show-toplevel"], executable=executable
    )
    try:
        top_level_text = top_level_output.decode("utf-8")
    except UnicodeDecodeError as error:
        raise GitObjectAuthorityError(
            "Git object authority top level is malformed"
        ) from error
    if top_level_text != f"{root}\n":
        raise GitObjectAuthorityError("Git object authority top level differs")

    head = _canonical_object_id(
        _git_read(
            root, ["rev-parse", "--verify", "HEAD^{commit}"], executable=executable
        ),
        "HEAD",
    )
    expected_head = values.get(SOURCE_SHA_VARIABLE, head)
    if re.fullmatch(r"[0-9a-f]{40}", expected_head) is None:
        raise GitObjectAuthorityError("source SHA is missing or malformed")
    if head != expected_head:
        raise GitObjectAuthorityError(
            "Git object authority HEAD differs from source SHA"
        )
    tree = _canonical_object_id(
        _git_read(
            root, ["rev-parse", "--verify", "HEAD^{tree}"], executable=executable
        ),
        "HEAD tree",
    )
    return GitObjectAuthority(root=root, head=head, tree=tree)


def historical_git(
    *arguments: str,
    repository: Path = REPOSITORY,
    environment: Mapping[str, str] | None = None,
) -> bytes:
    command = tuple(arguments)
    output_limit = _HISTORICAL_QUERIES.get(command)
    if output_limit is None:
        raise GitObjectAuthorityError(
            "Git object authority command is not a frozen read-only object query"
        )
    authority = git_object_authority(repository=repository, environment=environment)
    return _git_read(authority.root, arguments, output_limit=output_limit)


def git_object_authority_status(
    *,
    repository: Path = REPOSITORY,
    environment: Mapping[str, str] | None = None,
) -> bytes:
    authority = git_object_authority(repository=repository, environment=environment)
    return _git_read(
        authority.root,
        ["status", "--porcelain=v1", "--untracked-files=all"],
    )


def _load_checker():
    path = REPOSITORY / "tools/site/check_site.py"
    spec = importlib.util.spec_from_file_location("alpha2_site_check", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


checker = _load_checker()


def pinned_node_path(root: Path) -> str:
    directory = root / "pinned-node"
    directory.mkdir(parents=True, exist_ok=True)
    for name, version in (
        ("node", "v24.18.1"),
        ("npm", "11.16.0"),
        ("uv", "uv 0.12.1 (x86_64-unknown-linux-musl)"),
    ):
        executable = shutil.which(name)
        if executable is None:
            raise AssertionError(f"{name} is required by the site oracle")
        wrapper = directory / name
        wrapper.write_text(
            "#!/bin/sh\n"
            f"if [ \"${{1:-}}\" = --version ]; then printf '%s\\n' '{version}'; exit 0; fi\n"
            f'exec "{executable}" "$@"\n',
            encoding="utf-8",
        )
        wrapper.chmod(0o755)
    return f"{directory}{os.pathsep}{os.environ.get('PATH', '')}"


def _write_rustc_preflight_double(directory: Path) -> None:
    rustc = directory / "rustc"
    rustc.write_text(
        f"""#!{sys.executable}
from pathlib import Path
import os
import sys

args = sys.argv[1:]
if args == ["+stable", "-Vv"]:
    release = os.environ.get("FIXTURE_STABLE_RUST_RELEASE", "1.97.1")
    sys.stdout.write(f"fixture stable rustc\\nrelease: {{release}}\\n")
    raise SystemExit(0)
if args == ["-Vv"]:
    selected = os.environ.get("RUSTUP_TOOLCHAIN")
    if selected:
        trace = os.environ.get("FIXTURE_RUSTC_TRACE")
        if trace:
            with open(trace, "a", encoding="utf-8") as stream:
                stream.write(f"{{selected}}\\n")
        if selected in ("stable", "stable-x86_64-unknown-linux-gnu"):
            release = os.environ.get("FIXTURE_STABLE_RUST_RELEASE", "1.97.1")
        elif selected == "1.97.1-x86_64-unknown-linux-gnu":
            release = os.environ.get("FIXTURE_PINNED_RUST_RELEASE", "1.97.1")
        else:
            raise SystemExit(64)
        sys.stdout.write(f"fixture {{selected}} rustc\\nrelease: {{release}}\\n")
        raise SystemExit(0)
    try:
        toolchain = (Path.cwd() / "rust-toolchain.toml").read_bytes()
    except OSError:
        toolchain = None
    output = (
        (
            "fixture stable rustc\\nrelease: "
            + os.environ.get("FIXTURE_STABLE_RUST_RELEASE", "1.97.1")
            + "\\n"
        ).encode()
        if toolchain == {b'[toolchain]\nchannel = "stable"\ncomponents = ["rustfmt", "clippy"]\n'!r}
        else b"fixture selected rustc\\n"
    )
    sys.stdout.buffer.write(output)
    raise SystemExit(0)
raise SystemExit(64)
""",
        encoding="utf-8",
    )
    rustc.chmod(0o755)
    rustup = directory / "rustup"
    rustup.write_text(
        "#!/bin/sh\n"
        "test \"$#\" = 2\n"
        "test \"$1\" = toolchain\n"
        "test \"$2\" = list\n"
        "if test -n \"${FIXTURE_RUSTUP_TRACE:-}\"; then\n"
        "  printf 'toolchain list\\n' >> \"$FIXTURE_RUSTUP_TRACE\"\n"
        "fi\n"
        "printf '%s\\n' 'stable-x86_64-unknown-linux-gnu (active, default)' "
        "'1.97.1-x86_64-unknown-linux-gnu'\n",
        encoding="utf-8",
    )
    rustup.chmod(0o755)


def _write(path: Path, value: str | bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(value, bytes):
        path.write_bytes(value)
    else:
        path.write_text(value, encoding="utf-8")


def _workflow() -> str:
    return (REPOSITORY / ".github/workflows/pages.yml").read_text(encoding="utf-8")


def _runner() -> str:
    return """#!/usr/bin/env bash
export npm_config_offline=true CARGO_NET_OFFLINE=true UV_OFFLINE=1
export EQIORA_SITE_CARGO_VERSION=0.1.0-alpha.1 EQIORA_SITE_PYTHON_VERSION=0.1.0a1
python3 tools/site/check_site.py source-topology --root "$EQIORA_SITE_SOURCE_ROOT"
python3 tools/site/check_site.py browser-supply --site-root docs/site --browser-cache "$PLAYWRIGHT_BROWSERS_PATH" --expected-executable-sha256 "$EQIORA_SITE_BROWSER_SHA256" --expected-executable-bytes "$EQIORA_SITE_BROWSER_BYTES"
python3 -m unittest tools.site.tests.test_site_tools -v
python3 tools/site/build_products.py
python3 tools/docs/generate_interface_reference.py --repository . --eqiora-binary bin/eqiora --mcp-binary bin/eqiora-mcp --check
python3 tools/site/build_rust_reference.py --rustdoc-root rustdoc/doc --output rustdoc-stage
python3 tools/site/generate_evidence_catalog.py
python3 tools/site/check_site.py check
python3 tools/site/check_site.py serve
python3 tools/site/check_site.py source-topology --root "$EQIORA_SITE_SOURCE_ROOT"
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
    return ('<a class="site-title" href="/"><img src="/assets/brand.svg" alt=""><span>Eqiora</span></a>'
            '<nav class="sidebar"><a href="/get-started/">Docs</a><a href="/textbooks/">Textbooks</a>'
            '<a href="/gallery/">Gallery</a><a href="/reference/">Reference</a>'
            '<a href="/capabilities/">Capabilities</a><a href="/release-notes/">Releases</a>'
            '<a href="https://github.com/nkiyohara/eqiora">GitHub</a></nav>')


def _page(route: str, body: str) -> str:
    return f"<!doctype html><html>{_head(route)}<body><header>{_nav()}</header><main>{body}</main></body></html>"


def _exact_links() -> str:
    links = []
    for relative in (*checker.CASE_SOURCE_PATHS, *CASE_EVIDENCE_PATHS):
        url = f"https://github.com/nkiyohara/eqiora/blob/{SOURCE_SHA}/{relative}"
        label = Path(relative).parent.name + " " + Path(relative).name
        links.append(f'<a href="{url}">{label}</a>')
    return "".join(links)


def _case_body() -> str:
    nonclaims = " ".join(NONCLAIMS)
    return f"""<h1>Exact-cylinder steady Stokes</h1>
<p>Static walkthrough · canonical Python source available</p>
<p>{"one" + PUBLIC_CLAIM[3:]}</p>
<section><h2>Problem setup</h2><p>2.2m x 0.41m channel; centre [0.2,0.2]m; radius 0.05m.</p>
<span class="katex"><span class="katex-mathml"><math><mi>H</mi></math></span><span class="katex-html">H</span></span></section>
<section><h2>Eqiora model definition</h2>
<div class="eqiora-math-region" role="region" aria-label="Displayed equation" tabindex="0"><span class="katex-display"><span class="katex"><span class="katex-mathml"><math display="block"><mi>sigma</mi></math></span><span class="katex-html">sigma</span></span></span></div>
<p>Eqiora source form</p><pre>sigma(u,p) = 2 mu sym(grad(u)) - p I
-div(sigma(u,p)) - grad(phi) = 0
div(u) = 0</pre></section>
<section><h2>Mesh and boundaries</h2><p>{WITNESS_COPY}</p></section>
<section><h2>Submit and result</h2><p>One immutable common Plan and direct Result carrier.</p><a href="https://github.com/nkiyohara/eqiora/blob/{SOURCE_SHA}/examples/python/exact_cylinder_stokes.py#L45-L57">Eqiora source form: canonical Python resolve/run path</a></section>
<section><h2>Pressure visualization</h2><figure><img src="/assets/pressure.png" alt="{PRESSURE_ALT}"><figcaption>{checker.PRESSURE_CAPTION}</figcaption></figure><p>Presentation, not evidence.</p></section>
<section><h2>Verified and not claimed</h2><p>{nonclaims}</p>{_exact_links()}<a href="/capabilities/#exact-cylinder-steady-stokes">Read the human capability boundary</a></section>"""


def _home_body() -> str:
    return f"""<h1>Model meaning once. Realize it many ways.</h1>
<p>Eqiora is an open-source, meaning-first foundation for scientific modeling, simulation, differentiation, and execution.</p>
<p>Its central boundary is simple:</p>
<p>A model states typed mathematical relations. A realization chooses how those relations are discretized, solved, and executed.</p>
<p>That separation lets block diagrams, acausal physical networks, PDE fields, hybrid dynamics, and reusable components share one canonical meaning without making a numerical method or hardware backend part of the model.</p>
<a href="/get-started/">Get started</a><a href="/gallery/">Explore gallery</a>
<article><p>Featured walkthrough</p><h2>Exact-cylinder steady Stokes</h2><img src="/assets/pressure.png" alt="{PRESSURE_ALT}"><p>Follow one frozen 2D steady-Stokes problem from model definition and named boundaries through one submit/Result path to an independently admitted static pressure image.</p><p>Python</p><p>2D</p><p>steady Stokes</p><a href="/gallery/exact-cylinder-steady-stokes/">View the static walkthrough</a></article>
<article><h2>Docs</h2><p>Learn the Model–Realization boundary and start from bounded examples.</p></article>
<article><h2>Textbooks</h2><p>Follow the planned path from mathematics and physics to Eqiora models, numerical realization, and interpretation.</p></article>
<article><h2>Capabilities</h2><p>See what is available, executable, checked, or verified, with exact boundaries and non-claims.</p></article>
<article><h2>Reference</h2><p>Browse exact-commit Python, Rust, CLI, control-v2, and MCP surfaces. API presence is not verification or maturity.</p></article>
<p>Docs explains how to use Eqiora. Textbooks teach the mathematics, physics, and numerics. Gallery presents complete simulations. Reference records exact APIs and protocols. Capabilities states what runs and the boundary of each claim.</p>
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
        "/api/": "<h1>API</h1><p>Eqiora API overview.</p>",
        "/architecture/": "<h1>Architecture</h1><p>Eqiora architecture.</p>",
        "/capabilities/": '<h1>Capabilities</h1><p>Available Executable Checked Verified</p><h2>Thermal</h2><p>Exact boundary What this establishes Important non-claims</p><article id="exact-cylinder-steady-stokes"><h3>Exact-cylinder steady Stokes product path</h3></article><h2>Technical catalog</h2><a href="/evidence/#exact-packaged-steady-incompressible-stokes-component">Technical evidence entry</a>',
        "/concepts/": "<h1>Concepts</h1><p>Eqiora concepts.</p>",
        "/contributing/": "<h1>Contributing</h1><p>Contribute to Eqiora.</p>",
        "/evidence/": '<h1>Evidence catalog</h1><aside><h2>How to read the technical catalog</h2><p>Case Status Reference Conformance kit Target</p><a href="/capabilities/">human-readable Capabilities</a></aside><h2 id="exact-packaged-steady-incompressible-stokes-component">exact packaged steady incompressible Stokes component</h2>',
        "/examples/": '<h1>Examples</h1><a href="/gallery/">Gallery</a>',
        "/gallery/": '<h1>Gallery</h1><a href="/gallery/exact-cylinder-steady-stokes/">Exact-cylinder steady Stokes</a><a href="/gallery/mixed-boundary-elasticity/">Mixed-boundary linear elasticity</a>',
        "/gallery/exact-cylinder-steady-stokes/": _case_body(),
        "/gallery/mixed-boundary-elasticity/": "<h1>Mixed-boundary linear elasticity</h1><p>Static caller-owned displacement presentation.</p>",
        "/get-started/": "<h1>Get started</h1>",
        "/python/": "<h1>Python</h1><p>Eqiora Python reference.</p>",
        "/python/differentiation/": "<h1>Differentiation</h1><p>Python differentiation.</p>",
        "/python/execution-and-arrays/": "<h1>Execution and arrays</h1><p>Python execution and arrays.</p>",
        "/python/modeling/": "<h1>Modeling</h1><p>Python modeling.</p>",
        "/reference/": '<h1>Reference</h1><p>Python Rust CLI control-v2 MCP</p><p>API presence is not verification or maturity.</p><form action="/reference/"><input aria-label="Search"></form>',
        "/reference/cli/": "<h1>CLI</h1><p>eqiora check</p>",
        "/reference/control-v2/": "<h1>control-v2</h1><p>eqiora.control/v2</p>",
        "/reference/mcp/": "<h1>MCP</h1><p>eqiora.model.compile_check</p>",
        "/reference/python/": "<h1>Python reference</h1><p>Python API families.</p>",
        "/reference/python/diff/": "<h1>Diff reference</h1><p>Differentiation API.</p>",
        "/reference/python/eqiora/": "<h1>eqiora Python module</h1><p>Diagnostic</p>",
        "/reference/python/fluid/": "<h1>Fluid reference</h1><p>Fluid API.</p>",
        "/reference/python/fsi/": "<h1>FSI reference</h1><p>FSI API.</p>",
        "/reference/python/geometry/": "<h1>Geometry reference</h1><p>Geometry API.</p>",
        "/reference/python/jax/": "<h1>JAX reference</h1><p>JAX API.</p>",
        "/reference/python/matplotlib/": "<h1>Matplotlib reference</h1><p>Matplotlib API.</p>",
        "/reference/python/meshing/": "<h1>Meshing reference</h1><p>Meshing API.</p>",
        "/reference/python/solid/": "<h1>Solid reference</h1><p>Solid API.</p>",
        "/reference/python/torch/": "<h1>Torch reference</h1><p>Torch API.</p>",
        "/reference/python/trajectory/": "<h1>Trajectory reference</h1><p>Trajectory API.</p>",
        "/reference/rust/": '<h1>Rust reference</h1><p>eqiora::Diagnostic stable eqiora::api::CadBoxIntentV1 transitional eqiora::api module</p><a href="/reference/rust/api/eqiora/struct.Diagnostic.html">Diagnostic</a>',
        "/release-notes/": "<h1>Release notes</h1><p>Eqiora release notes.</p>",
        "/textbooks/": "<h1>Textbooks</h1><h2>Foundations</h2><h2>Physics</h2><h2>Advanced study</h2><p>In progress 0 executable simulation chapters current alpha</p><a href=\"/textbooks/mathematical-modeling/\">Open the series map</a><a href=\"/textbooks/numerical-simulation/\">Open the series map</a><a href=\"/textbooks/fluid-mechanics-cfd/\">Open the series map</a><a href=\"/textbooks/structural-mechanics-fem/\">Open the series map</a><a href=\"/textbooks/heat-mass-transfer/\">Open the series map</a><a href=\"/textbooks/circuits-dynamics-hybrid/\">Open the series map</a><a href=\"/gallery/exact-cylinder-steady-stokes/\">Preview a related bounded Gallery walkthrough</a>",
        "/textbooks/circuits-dynamics-hybrid/": "<h1>Circuits, Dynamics, and Hybrid Systems</h1><p>Planned 0 executable chapters</p><h2>Chapter map</h2><h2>Publication boundary</h2>",
        "/textbooks/fluid-mechanics-cfd/": "<h1>Fluid Mechanics and Computational Fluid Dynamics</h1><p>Planned 0 executable chapters</p><h2>Chapter map</h2><h2>Publication boundary</h2>",
        "/textbooks/heat-mass-transfer/": "<h1>Heat and Mass Transfer</h1><p>Planned 0 executable chapters</p><h2>Chapter map</h2><h2>Publication boundary</h2>",
        "/textbooks/mathematical-modeling/": "<h1>Mathematical Modeling with Eqiora</h1><p>In progress 0 executable simulation chapters</p><h2>Chapter map</h2><a href=\"/textbooks/mathematical-modeling/models-not-simulations/\">Models are not simulations</a><a href=\"/textbooks/mathematical-modeling/quantities-dimensions-units/\">Quantities, dimensions, and units</a><h2>Publication boundary</h2>",
        "/textbooks/mathematical-modeling/models-not-simulations/": "<h1>Models are not simulations</h1><p>Illustrative</p><h2>Learning outcomes</h2><h2>Observation boundary</h2><h2>Deliberate failure</h2><h2>Exercises</h2><h2>Non-claims</h2><a href=\"/textbooks/mathematical-modeling/\">Back to the series map</a>",
        "/textbooks/mathematical-modeling/quantities-dimensions-units/": "<h1>Quantities, dimensions, and units</h1><p>Illustrative</p><h2>Learning outcomes</h2><h2>Observation boundary</h2><h2>Deliberate failure</h2><h2>Exercises</h2><h2>Non-claims</h2><a href=\"/textbooks/mathematical-modeling/\">Back to the series map</a>",
        "/textbooks/numerical-simulation/": "<h1>Numerical Simulation with Eqiora</h1><p>Planned 0 executable chapters</p><h2>Chapter map</h2><h2>Publication boundary</h2>",
        "/textbooks/structural-mechanics-fem/": "<h1>Structural Mechanics and the Finite Element Method</h1><p>Planned 0 executable chapters</p><h2>Chapter map</h2><h2>Publication boundary</h2>",
        "/404.html": "<h1>Page not found</h1>",
    }
    assert tuple(pages) == SITE_ROUTES
    for route, body in pages.items():
        if route == "/":
            relative = Path("index.html")
        elif route == "/404.html":
            relative = Path("404.html")
        else:
            relative = Path(route.removeprefix("/")) / "index.html"
        canonical = "/404/" if route == "/404.html" else route
        _write(artifact / relative, _page(canonical, body))
    _write(
        artifact / "reference/rust/api/eqiora/struct.Diagnostic.html",
        '<!doctype html><html><body><main><h1>Struct Diagnostic</h1><a href="index.html">eqiora</a></main></body></html>',
    )
    _write(
        artifact / "reference/rust/api/eqiora/index.html",
        '<!doctype html><html><body><main><h1>Crate eqiora</h1><a href="struct.Diagnostic.html">Diagnostic</a></main></body></html>',
    )
    urls = "".join(
        f"<url><loc>https://eqiora.org{route}</loc></url>" for route in SITE_ROUTES[:-1]
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
    _write(
        root / "tools/ci/classify_changes.py",
        (REPOSITORY / "tools/ci/classify_changes.py").read_text(encoding="utf-8"),
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
