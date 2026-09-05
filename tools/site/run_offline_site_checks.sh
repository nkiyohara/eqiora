#!/usr/bin/env bash
set -euo pipefail
if test "$#" -ne 0; then
  echo "usage: $0" >&2
  exit 2
fi
for variable in \
  EQIORA_API_SCRATCH \
  EQIORA_SITE_SOURCE_ROOT \
  EQIORA_SITE_GIT_OBJECT_REPOSITORY \
  EQIORA_SITE_ASTRO_OUT_DIR \
  EQIORA_SITE_CARGO_TARGET \
  EQIORA_SITE_RUSTDOC_TARGET \
  EQIORA_SITE_RUSTDOC_STAGE \
  EQIORA_SITE_ARTIFACT \
  EQIORA_SITE_SOURCE_SHA \
  EQIORA_SITE_BROWSER_SHA256 \
  EQIORA_SITE_BROWSER_BYTES \
  PLAYWRIGHT_BROWSERS_PATH
do
  test -n "${!variable:-}"
done
test "$LC_ALL" = C
test "$TZ" = UTC
test -d "$EQIORA_SITE_SOURCE_ROOT"
test ! -L "$EQIORA_SITE_SOURCE_ROOT"
test -d "$EQIORA_API_SCRATCH"
test ! -L "$EQIORA_API_SCRATCH"
scratch_real="$(realpath "$EQIORA_API_SCRATCH")"
source_real="$(realpath "$EQIORA_SITE_SOURCE_ROOT")"
authority_real="$(realpath "$EQIORA_SITE_GIT_OBJECT_REPOSITORY")"
test "$authority_real" = "$EQIORA_SITE_GIT_OBJECT_REPOSITORY"
test ! -L "$EQIORA_SITE_GIT_OBJECT_REPOSITORY"
test "$authority_real" != "$source_real"
test "$scratch_real" = "$EQIORA_API_SCRATCH"
test "$source_real" = "$EQIORA_SITE_SOURCE_ROOT"
[[ "$EQIORA_SITE_SOURCE_SHA" =~ ^[0-9a-f]{40}$ ]]
test "$EQIORA_SITE_SOURCE_ROOT" = "$EQIORA_API_SCRATCH/source"
test "$EQIORA_SITE_ASTRO_OUT_DIR" = "$EQIORA_API_SCRATCH/astro"
test "$EQIORA_SITE_CARGO_TARGET" = "$EQIORA_API_SCRATCH/cargo-target"
test "$EQIORA_SITE_RUSTDOC_TARGET" = "$EQIORA_API_SCRATCH/rustdoc-target"
test "$EQIORA_SITE_RUSTDOC_STAGE" = "$EQIORA_API_SCRATCH/rustdoc-stage"
test "$EQIORA_SITE_ARTIFACT" = "$EQIORA_API_SCRATCH/build/site"
case "$PLAYWRIGHT_BROWSERS_PATH" in */eqiora-pw-1.62.1-r1234) ;; *) exit 1 ;; esac
test -d "$EQIORA_API_SCRATCH/build"
test ! -L "$EQIORA_API_SCRATCH/build"
test -d "$EQIORA_API_SCRATCH/uv-cache"
test ! -L "$EQIORA_API_SCRATCH/uv-cache"
test -z "$(find "$EQIORA_API_SCRATCH/build" -mindepth 1 -print -quit)"
test "$(find "$EQIORA_API_SCRATCH" -mindepth 1 -maxdepth 1 -printf '%f\n' | LC_ALL=C sort)" = $'build\nsource\nuv-cache'
test ! -e "$EQIORA_SITE_SOURCE_ROOT/.git"
test -f "$EQIORA_SITE_SOURCE_ROOT/Cargo.toml"
test -f "$EQIORA_SITE_SOURCE_ROOT/docs/site/package.json"
test -d "$EQIORA_SITE_SOURCE_ROOT/docs/site/node_modules"
test ! -L "$EQIORA_SITE_SOURCE_ROOT/docs/site/node_modules"
test -x "$EQIORA_SITE_SOURCE_ROOT/tools/site/run_offline_site_checks.sh"
for output in \
  "$EQIORA_SITE_ASTRO_OUT_DIR" \
  "$EQIORA_SITE_CARGO_TARGET" \
  "$EQIORA_SITE_RUSTDOC_TARGET" \
  "$EQIORA_SITE_RUSTDOC_STAGE" \
  "$EQIORA_SITE_ARTIFACT"
do
  test ! -e "$output"
  test ! -L "$output"
done
test "$npm_config_offline" = true
test "$CARGO_NET_OFFLINE" = true
test "$UV_OFFLINE" = 1
test "$(uname -m)" = x86_64
test "$(node --version)" = v24.18.1
test "$(npm --version)" = 11.16.0
test "$(python3 --version)" = "Python 3.13.14"
test "$(uv --version)" = "uv 0.12.1 (x86_64-unknown-linux-musl)"
required_rust_release=1.97.1
rustc_has_required_release() {
  local version_output="$1"
  local -a release_lines=()
  mapfile -t release_lines < <(
    printf '%s\n' "$version_output" | grep '^release: ' || true
  )
  test "${#release_lines[@]}" = 1
  test "${release_lines[0]}" = "release: $required_rust_release"
}
selected_rust_toolchain=
rustup_proxy_bin="$HOME/.cargo/bin"
test -x "$rustup_proxy_bin/rustup"
test -x "$rustup_proxy_bin/rustc"
if stable_version="$(rustc +stable -Vv 2>/dev/null)"; then
  source_selected_version="$(env -u RUSTUP_TOOLCHAIN rustc -Vv)"
  test "$source_selected_version" = "$stable_version"
  if rustc_has_required_release "$stable_version"; then
    selected_rust_toolchain=stable
  fi
fi
if test -z "$selected_rust_toolchain"; then
  installed_toolchain_output="$("$rustup_proxy_bin/rustup" toolchain list)"
  mapfile -t installed_toolchain_lines <<< "$installed_toolchain_output"
  for installed_toolchain_line in "${installed_toolchain_lines[@]}"; do
    if [[ ! "$installed_toolchain_line" =~ ^([A-Za-z0-9._-]+)([[:space:]]+\([^()]+\))?$ ]]; then
      echo "offline site checks: malformed installed Rust toolchain entry" >&2
      exit 1
    fi
    installed_toolchain="${BASH_REMATCH[1]}"
    if installed_version="$(
      RUSTUP_TOOLCHAIN="$installed_toolchain" \
        "$rustup_proxy_bin/rustc" -Vv 2>/dev/null
    )" && rustc_has_required_release "$installed_version"
    then
      selected_rust_toolchain="$installed_toolchain"
      break
    fi
  done
fi
if test -z "$selected_rust_toolchain"; then
  echo "offline site checks: no installed Rust toolchain reports release $required_rust_release" >&2
  exit 1
fi
export RUSTUP_TOOLCHAIN="$selected_rust_toolchain"
export PATH="$rustup_proxy_bin:$PATH"
selected_rust_version="$(rustc -Vv)"
rustc_has_required_release "$selected_rust_version"
export PYTHONDONTWRITEBYTECODE=1
source_manifest_before="$EQIORA_API_SCRATCH/source-sha256.before"
source_manifest_after="$EQIORA_API_SCRATCH/source-sha256.after"
cd "$EQIORA_SITE_SOURCE_ROOT"
python3 tools/editor/check_syntax_bundle.py
python3 -m unittest discover -s tools/editor/tests -p 'test_*.py' -v
python3 tools/site/check_site.py source-topology --root "$EQIORA_SITE_SOURCE_ROOT"
find "$EQIORA_SITE_SOURCE_ROOT" \
  -path "$EQIORA_SITE_SOURCE_ROOT/docs/site/node_modules" -prune -o \
  -path "$EQIORA_SITE_SOURCE_ROOT/docs/site/.astro" -prune -o \
  -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > "$source_manifest_before"
python3 - <<'PY'
import socket, urllib.request
for action, label in (
    (lambda: socket.getaddrinfo("example.com", 443), "external DNS sentinel"),
    (lambda: socket.create_connection(("1.1.1.1", 443), 2), "external TCP sentinel"),
    (lambda: urllib.request.urlopen("https://example.com/", timeout=2), "external fetch sentinel"),
):
    try: action()
    except OSError: pass
    else: raise SystemExit(f"{label} unexpectedly succeeded")
PY
python3 tools/site/check_site.py browser-supply \
  --site-root docs/site --browser-cache "$PLAYWRIGHT_BROWSERS_PATH" \
  --expected-executable-sha256 "$EQIORA_SITE_BROWSER_SHA256" \
  --expected-executable-bytes "$EQIORA_SITE_BROWSER_BYTES"
dpkg-query -W libclang-dev libffi-dev libopenmpi-dev openmpi-bin >/dev/null
# The oracle package proves its ordinary synthetic path before any mutant or product check.
PYTHONPATH=tools/site/tests/site \
python3 -m unittest \
  test_contract.CompleteContractTests.test_00_synthetic_ordinary_site_passes_before_mutants \
  -v
unittest_home="$HOME/../$(basename "$HOME")"
test "$(realpath "$unittest_home")" = "$(realpath "$HOME")"
HOME="$unittest_home" \
python3 -m unittest discover -s tools/site/tests/site -p 'test_*.py' -v
python3 -m unittest tools.site.tests.test_site_tools -v
# Real-source provider, identity, supply, and trigger gates precede every consumer build.
python3 - <<'PY'
from pathlib import Path

from tools.site.check_site import check_source

errors = check_source(Path.cwd())
if errors:
    for error in errors:
        print(f"site source: {error}")
    raise SystemExit(1)
PY
read -r cargo_version python_version < <(
  python3 - <<'PY'
import importlib.util
import sys
import tomllib
from pathlib import Path

root = Path.cwd()
cargo = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
path = root / "tools/release/python_candidate_common.py"
spec = importlib.util.spec_from_file_location("eqiora_release_version", path)
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
print(cargo, module.python_distribution_version(cargo))
PY
)
cargo_target="$EQIORA_SITE_CARGO_TARGET"
wheels="$EQIORA_API_SCRATCH/wheels"
venv="$EQIORA_API_SCRATCH/venv"
identity_cwd="$EQIORA_API_SCRATCH/identity-cwd"
build_receipt="$EQIORA_API_SCRATCH/build-products.json"
for path in "$cargo_target" "$wheels" "$venv" "$identity_cwd" "$build_receipt" \
  "$EQIORA_SITE_ASTRO_OUT_DIR" "$EQIORA_SITE_RUSTDOC_STAGE" \
  "$EQIORA_SITE_ARTIFACT"
do
  test ! -e "$path"
  test ! -L "$path"
done
mkdir "$wheels" "$identity_cwd"
RUSTDOCFLAGS="-D warnings --html-in-header docs/site/src/reference/rustdoc-head.html --extend-css docs/site/src/styles/rustdoc.css" \
python3 tools/site/build_products.py \
  --scratch-root "$EQIORA_API_SCRATCH" \
  --receipt "$build_receipt" \
  --cargo "$rustup_proxy_bin/cargo" \
  --rust-toolchain "$selected_rust_toolchain"
eqiora_binary="$cargo_target/release/eqiora"
mcp_binary="$cargo_target/release/eqiora-mcp"
test -x "$eqiora_binary"
test -x "$mcp_binary"
test "$($eqiora_binary --version)" = "eqiora $cargo_version"
test -d "$wheels"; test ! -L "$wheels"
mapfile -d '' wheel_entries < <(find "$wheels" -mindepth 1 -maxdepth 1 -print0)
mapfile -d '' wheel_files < <(find "$wheels" -mindepth 1 -maxdepth 1 -name '*.whl' -print0)
test "${#wheel_entries[@]}" = 2; test "${#wheel_files[@]}" = 1
wheel="${wheel_files[0]}"; control="$wheels/.gitignore"
test "$(basename "$wheel")" != .whl
for output in "$wheel" "$control"; do
  test -f "$output"; test ! -L "$output"; test "$(stat -c %h "$output")" = 1
done
test -s "$wheel"
test "$(stat -c %s "$control")" = 1
test "$(sha256sum "$control" | cut -d ' ' -f 1)" = 684888c0ebb17f374298b65ee2807526c066094c701bcc7ebbe1c1095f494fc1
wheel_identity="$(stat -c %d:%i "$wheel") $(sha256sum "$wheel")"; control_identity="$(stat -c %d:%i "$control") $(sha256sum "$control")"
uv venv --python 3.13 --no-python-downloads "$venv"
test "$(find "$wheels" -mindepth 1 -maxdepth 1 -printf '.\n' | wc -l)" = 2
for output in "$wheel" "$control"; do test -f "$output"; test ! -L "$output"; test "$(stat -c %h "$output")" = 1; done
test "$(stat -c %d:%i "$wheel") $(sha256sum "$wheel")" = "$wheel_identity"
test "$(stat -c %d:%i "$control") $(sha256sum "$control")" = "$control_identity"
uv pip install --python "$venv/bin/python" --no-index --no-deps "$wheel"
(
  cd "$identity_cwd"
  env -u PYTHONPATH PYTHONNOUSERSITE=1 PYTHONDONTWRITEBYTECODE=1 LC_ALL=C \
    "$venv/bin/python" -I - "$python_version" "$venv" <<'PY'
import importlib.metadata
import importlib.machinery
import importlib.util
import importlib
import pathlib
import sys

expected = sys.argv[1]
venv = pathlib.Path(sys.argv[2]).resolve()
import eqiora
import eqiora._eqiora as native

assert eqiora.__version__ == expected
assert native.__version__ == expected
assert importlib.metadata.version("eqiora") == expected
public_file = pathlib.Path(eqiora.__file__)
native_file = pathlib.Path(native.__file__)
package = public_file.resolve().parent
assert not public_file.is_symlink() and public_file.resolve().is_relative_to(venv)
assert not native_file.is_symlink() and native_file.resolve().is_relative_to(venv)
assert native.__name__ == "eqiora._eqiora"
assert native_file.resolve().parent == package
assert any(native_file.name.endswith(suffix) for suffix in importlib.machinery.EXTENSION_SUFFIXES)
distribution = importlib.metadata.distribution("eqiora")
assert pathlib.Path(distribution.locate_file("")).resolve().is_relative_to(venv)
assert importlib.util.find_spec("_eqiora") is None
try:
    importlib.import_module("_eqiora")
except ModuleNotFoundError as error:
    assert error.name == "_eqiora"
else:
    raise AssertionError("top-level _eqiora unexpectedly exists")
PY
)
# The committed evidence projection is checked before API projection or Astro.
"$cargo_target/release/eqiora-verify" \
  index --format json > "$EQIORA_API_SCRATCH/evidence-index.json"
python3 tools/site/generate_evidence_catalog.py \
  --input "$EQIORA_API_SCRATCH/evidence-index.json" \
  --output "$EQIORA_API_SCRATCH/evidence-index.mdx"
cmp --silent \
  "$EQIORA_API_SCRATCH/evidence-index.mdx" \
  docs/site/src/content/docs/evidence/index.mdx
python3 tools/site/generate_evidence_catalog.py \
  --check \
  --input "$EQIORA_API_SCRATCH/evidence-index.json" \
  --output docs/site/src/content/docs/evidence/index.mdx
python3 tools/docs/generate_python_api.py --check
python3 tools/docs/generate_interface_reference.py \
  --repository "$EQIORA_SITE_SOURCE_ROOT" \
  --eqiora-binary "$eqiora_binary" \
  --mcp-binary "$mcp_binary" \
  --source-sha "$EQIORA_SITE_SOURCE_SHA" \
  --check
"$cargo_target/release/xtask" check-facade
mkdir "$EQIORA_SITE_RUSTDOC_STAGE"
python3 tools/site/build_rust_reference.py \
  --rustdoc-root "$EQIORA_SITE_RUSTDOC_TARGET/doc" \
  --output "$EQIORA_SITE_RUSTDOC_STAGE"
rustdoc_handoff="$EQIORA_SITE_RUSTDOC_STAGE/reference/rust/api"
for path in "$EQIORA_SITE_RUSTDOC_STAGE" "$EQIORA_SITE_RUSTDOC_STAGE/reference" \
  "$EQIORA_SITE_RUSTDOC_STAGE/reference/rust" "$rustdoc_handoff" "$rustdoc_handoff/eqiora"
do
  test -d "$path"; test ! -L "$path"
done
test "$(realpath "$rustdoc_handoff")" = "$rustdoc_handoff"
test -f "$rustdoc_handoff/eqiora/index.html"; test ! -L "$rustdoc_handoff/eqiora/index.html"
test ! -e "$rustdoc_handoff/eqiora_mcp"; test ! -L "$rustdoc_handoff/eqiora_mcp"
EQIORA_SITE_BUILD_PROFILE=complete \
EQIORA_SITE_SOURCE_SHA="$EQIORA_SITE_SOURCE_SHA" \
EQIORA_SITE_CARGO_VERSION="$cargo_version" \
EQIORA_SITE_PYTHON_VERSION="$python_version" \
EQIORA_SITE_ASTRO_OUT_DIR="$EQIORA_SITE_ASTRO_OUT_DIR" \
npm --prefix docs/site run build
assembly_scratch="$EQIORA_API_SCRATCH/assembly"
mkdir "$assembly_scratch"
python3 tools/site/assemble_site.py \
  --astro-root "$EQIORA_SITE_ASTRO_OUT_DIR" \
  --rustdoc-root "$rustdoc_handoff" \
  --control-schema schemas/control/compile-v2.schema.json \
  --output "$EQIORA_SITE_ARTIFACT" \
  --scratch-root "$assembly_scratch"
python3 tools/site/check_site.py check \
  --root "$EQIORA_SITE_SOURCE_ROOT" \
  --artifact "$EQIORA_SITE_ARTIFACT" \
  --source-sha "$EQIORA_SITE_SOURCE_SHA"

server_log="$EQIORA_API_SCRATCH/site-server.log"
python3 tools/site/check_site.py serve \
  --artifact "$EQIORA_SITE_ARTIFACT" \
  --host 127.0.0.1 \
  --port 4173 >"$server_log" 2>&1 &
server_pid=$!
cleanup_server() {
  kill "$server_pid" 2>/dev/null || true
  wait "$server_pid" 2>/dev/null || true
}
trap cleanup_server EXIT
python3 - <<'PY'
import time
import urllib.request

for _ in range(100):
    try:
        with urllib.request.urlopen("http://127.0.0.1:4173/", timeout=1) as response:
            assert response.status == 200
            assert b"Eqiora" in response.read()
            break
    except OSError:
        time.sleep(0.05)
else:
    raise SystemExit("loopback site sentinel did not become ready")
PY

EQIORA_SITE_BASE_URL=http://127.0.0.1:4173 \
npx --prefix docs/site playwright test --config docs/site/playwright.config.ts
cleanup_server
trap - EXIT

python3 tools/site/check_site.py source-topology --root "$EQIORA_SITE_SOURCE_ROOT"
find "$EQIORA_SITE_SOURCE_ROOT" \
  -path "$EQIORA_SITE_SOURCE_ROOT/docs/site/node_modules" -prune -o \
  -path "$EQIORA_SITE_SOURCE_ROOT/docs/site/.astro" -prune -o \
  -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > "$source_manifest_after"
cmp --silent "$source_manifest_before" "$source_manifest_after"

echo "offline site checks: exact artifact, browser, and accessibility contract passed"
