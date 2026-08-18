#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  "") ;;
  --preflight-only) ;;
  *) echo "usage: $0 [--preflight-only]" >&2; exit 2 ;;
esac
test "$#" -le 1

for variable in \
  EQIORA_API_SCRATCH \
  EQIORA_SITE_SOURCE_ROOT \
  EQIORA_SITE_ASTRO_OUT_DIR \
  EQIORA_SITE_RUSTDOC_TARGET \
  EQIORA_SITE_RUSTDOC_STAGE \
  EQIORA_SITE_ARTIFACT \
  EQIORA_SITE_SOURCE_SHA \
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
test "$scratch_real" = "$EQIORA_API_SCRATCH"
test "$source_real" = "$EQIORA_SITE_SOURCE_ROOT"
[[ "$EQIORA_SITE_SOURCE_SHA" =~ ^[0-9a-f]{40}$ ]]
test "$EQIORA_SITE_SOURCE_ROOT" = "$EQIORA_API_SCRATCH/source"
test "$EQIORA_SITE_ASTRO_OUT_DIR" = "$EQIORA_API_SCRATCH/astro"
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
  "$EQIORA_SITE_RUSTDOC_TARGET" \
  "$EQIORA_SITE_RUSTDOC_STAGE" \
  "$EQIORA_SITE_ARTIFACT"
do
  test ! -e "$output"
  test ! -L "$output"
done

if test "${1:-}" = --preflight-only; then
  exit 0
fi

test "$npm_config_offline" = true
test "$CARGO_NET_OFFLINE" = true
test "$UV_OFFLINE" = 1
test "$(uname -m)" = x86_64
test "$(node --version)" = v24.18.1
test "$(npm --version)" = 11.16.0
test "$(python3 --version)" = "Python 3.13.14"
test "$(uv --version)" = "uv 0.12.1 (x86_64-unknown-linux-musl)"
test "$(rustc -Vv)" = "$(rustc +stable -Vv)"
export PYTHONDONTWRITEBYTECODE=1

source_manifest_before="$EQIORA_API_SCRATCH/source-sha256.before"
source_manifest_after="$EQIORA_API_SCRATCH/source-sha256.after"
test -z "$(find "$EQIORA_SITE_SOURCE_ROOT" \
  -path "$EQIORA_SITE_SOURCE_ROOT/docs/site/node_modules" -prune -o \
  -type l -print -quit)"
find "$EQIORA_SITE_SOURCE_ROOT" \
  -path "$EQIORA_SITE_SOURCE_ROOT/docs/site/node_modules" -prune -o \
  -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > "$source_manifest_before"

cd "$EQIORA_SITE_SOURCE_ROOT"

python3 - <<'PY'
import socket
import urllib.request

try:
    socket.getaddrinfo("example.com", 443)
except OSError:
    pass
else:
    raise SystemExit("external DNS sentinel unexpectedly succeeded")

sock = socket.socket()
sock.settimeout(2)
try:
    sock.connect(("1.1.1.1", 443))
except OSError:
    pass
else:
    raise SystemExit("external TCP sentinel unexpectedly succeeded")
finally:
    sock.close()

try:
    urllib.request.urlopen("https://example.com/", timeout=2)
except OSError:
    pass
else:
    raise SystemExit("external fetch sentinel unexpectedly succeeded")
PY

node --input-type=module <<'NODE'
import { readFileSync } from 'node:fs';
import { chromium } from './docs/site/node_modules/@playwright/test/index.mjs';

const browsers = JSON.parse(readFileSync('./docs/site/node_modules/playwright-core/browsers.json', 'utf8'));
for (const name of ['chromium', 'chromium-headless-shell']) {
  const browser = browsers.browsers.find((entry) => entry.name === name);
  if (!browser || browser.revision !== '1234' || browser.browserVersion !== '151.0.7922.34') {
    throw new Error(`unexpected ${name} identity: ${JSON.stringify(browser)}`);
  }
}
const executable = chromium.executablePath();
if (!executable.startsWith(`${process.env.PLAYWRIGHT_BROWSERS_PATH}/`)) {
  throw new Error(`Playwright executable escaped the named browser supply: ${executable}`);
}
NODE

browser_executable="$({ cd docs/site; node --input-type=module -e \
  "import { chromium } from '@playwright/test'; console.log(chromium.executablePath())"; })"
test -x "$browser_executable"
test "$($browser_executable --version)" = "HeadlessChrome 151.0.7922.34"
dpkg-query -W libclang-dev libffi-dev libopenmpi-dev openmpi-bin >/dev/null

# The oracle package proves its ordinary synthetic path before any mutant or product check.
PYTHONPATH=tools/site/tests/site \
python3 -m unittest \
  test_contract.CompleteContractTests.test_00_synthetic_ordinary_site_passes_before_mutants \
  -v
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

cargo_target="$EQIORA_API_SCRATCH/cargo-target"
python_target="$EQIORA_API_SCRATCH/python-target"
wheels="$EQIORA_API_SCRATCH/wheels"
venv="$EQIORA_API_SCRATCH/venv"
identity_cwd="$EQIORA_API_SCRATCH/identity-cwd"
for path in "$cargo_target" "$python_target" "$wheels" "$venv" "$identity_cwd" \
  "$EQIORA_SITE_ASTRO_OUT_DIR" "$EQIORA_SITE_RUSTDOC_TARGET" \
  "$EQIORA_SITE_RUSTDOC_STAGE" "$EQIORA_SITE_ARTIFACT"
do
  test ! -e "$path"
  test ! -L "$path"
done
mkdir "$wheels" "$identity_cwd"

cargo +stable build --locked --release -p eqiora \
  --bin eqiora --bin eqiora-mcp \
  --target-dir "$cargo_target"
eqiora_binary="$cargo_target/release/eqiora"
mcp_binary="$cargo_target/release/eqiora-mcp"
test -x "$eqiora_binary"
test -x "$mcp_binary"
test "$($eqiora_binary --version)" = "eqiora $cargo_version"

CARGO_TARGET_DIR="$python_target" \
uv build --wheel --clear --python 3.13 --no-python-downloads \
  --cache-dir "$EQIORA_API_SCRATCH/uv-cache" \
  --out-dir "$wheels" .
mapfile -d '' wheel_files < <(find "$wheels" -maxdepth 1 -type f -name '*.whl' -print0)
test "${#wheel_files[@]}" = 1
test "$(find "$wheels" -mindepth 1 -maxdepth 1 -print | wc -l)" = 1
uv venv --python 3.13 --no-python-downloads "$venv"
uv pip install --python "$venv/bin/python" --no-index --no-deps "${wheel_files[0]}"

(
  cd "$identity_cwd"
  env -u PYTHONPATH PYTHONNOUSERSITE=1 PYTHONDONTWRITEBYTECODE=1 LC_ALL=C \
    "$venv/bin/python" -I - "$python_version" "$venv" <<'PY'
import importlib.metadata
import pathlib
import sys

expected = sys.argv[1]
venv = pathlib.Path(sys.argv[2]).resolve()
import eqiora
import _eqiora

assert eqiora.__version__ == expected
assert _eqiora.__version__ == expected
assert importlib.metadata.version("eqiora") == expected
assert pathlib.Path(eqiora.__file__).resolve().is_relative_to(venv)
distribution = importlib.metadata.distribution("eqiora")
assert pathlib.Path(distribution.locate_file("")).resolve().is_relative_to(venv)
PY
)

# The committed evidence projection is checked before API projection or Astro.
cargo +stable run --locked --quiet -p eqiora-verify -- \
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
  --check
cargo +stable xtask check-facade

RUSTDOCFLAGS="-D warnings --html-in-header docs/site/src/reference/rustdoc-head.html --extend-css docs/site/src/styles/rustdoc.css" \
cargo +stable doc --locked -p eqiora --lib --no-deps --all-features \
  --target-dir "$EQIORA_SITE_RUSTDOC_TARGET"
mkdir "$EQIORA_SITE_RUSTDOC_STAGE"
python3 tools/site/build_rust_reference.py \
  --rustdoc-root "$EQIORA_SITE_RUSTDOC_TARGET/doc" \
  --output "$EQIORA_SITE_RUSTDOC_STAGE"

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
  --rustdoc-root "$EQIORA_SITE_RUSTDOC_STAGE" \
  --control-schema schemas/control/compile-v2.schema.json \
  --output "$EQIORA_SITE_ARTIFACT" \
  --scratch-root "$assembly_scratch"

python3 tools/site/check_site.py check \
  --root "$EQIORA_SITE_SOURCE_ROOT" \
  --artifact "$EQIORA_SITE_ARTIFACT" \
  --source-sha "$EQIORA_SITE_SOURCE_SHA"

# A complete ordinary Astro build must pass before the invalid-TeX falsifier runs.
invalid_repository="$EQIORA_API_SCRATCH/invalid-math-repository"
invalid_site="$invalid_repository/docs/site"
invalid_output="$EQIORA_API_SCRATCH/invalid-math-output"
invalid_log="$EQIORA_API_SCRATCH/invalid-math.log"
python3 - "$EQIORA_SITE_SOURCE_ROOT" "$invalid_repository" <<'PY'
import shutil
import sys
from pathlib import Path

source = Path(sys.argv[1])
destination = Path(sys.argv[2])
shutil.copytree(
    source,
    destination,
    symlinks=True,
)
PY
python3 - "$invalid_site/src/content/docs/__invalid_math_sentinel__.mdx" <<'PY'
import sys
from pathlib import Path

Path(sys.argv[1]).write_text(
    "---\ntitle: Invalid math sentinel\n---\n\n$$\n\\frac{\n$$\n",
    encoding="utf-8",
)
PY
if EQIORA_SITE_BUILD_PROFILE=complete \
  EQIORA_SITE_SOURCE_SHA="$EQIORA_SITE_SOURCE_SHA" \
  EQIORA_SITE_CARGO_VERSION="$cargo_version" \
  EQIORA_SITE_PYTHON_VERSION="$python_version" \
  EQIORA_SITE_ASTRO_OUT_DIR="$invalid_output" \
  npm --prefix "$invalid_site" run build >"$invalid_log" 2>&1
then
  echo "invalid TeX unexpectedly built successfully" >&2
  exit 1
fi
grep -Fq '__invalid_math_sentinel__.mdx' "$invalid_log"
grep -Eiq '(KaTeX.*(parse error|ParseError|Expected)|(parse error|ParseError|Expected).*KaTeX)' "$invalid_log"

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

test -z "$(find "$EQIORA_SITE_SOURCE_ROOT" \
  -path "$EQIORA_SITE_SOURCE_ROOT/docs/site/node_modules" -prune -o \
  -type l -print -quit)"
find "$EQIORA_SITE_SOURCE_ROOT" \
  -path "$EQIORA_SITE_SOURCE_ROOT/docs/site/node_modules" -prune -o \
  -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > "$source_manifest_after"
cmp --silent "$source_manifest_before" "$source_manifest_after"

echo "offline site checks: exact artifact, browser, and accessibility contract passed"
