#!/usr/bin/env bash
set -euo pipefail

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

test "$npm_config_offline" = true
test "$CARGO_NET_OFFLINE" = true
test "$UV_OFFLINE" = 1
test "$LC_ALL" = C
test "$TZ" = UTC
test "$(uname -m)" = x86_64
test "$(node --version)" = v24.18.1
test "$(npm --version)" = 11.16.0
test "$(python3 --version)" = "Python 3.13.14"
test "$(uv --version)" = "uv 0.12.1 (x86_64-unknown-linux-musl)"
test "$(rustc -Vv)" = "$(rustc +stable -Vv)"
test -d "$EQIORA_SITE_SOURCE_ROOT"
test ! -L "$EQIORA_SITE_SOURCE_ROOT"
test -d "$EQIORA_API_SCRATCH"
test ! -L "$EQIORA_API_SCRATCH"
test "$(realpath "$EQIORA_SITE_SOURCE_ROOT")" = "$EQIORA_SITE_SOURCE_ROOT"
test "$(realpath "$EQIORA_API_SCRATCH")" = "$EQIORA_API_SCRATCH"
[[ "$EQIORA_SITE_SOURCE_SHA" =~ ^[0-9a-f]{40}$ ]]
case "$EQIORA_SITE_ASTRO_OUT_DIR" in "$EQIORA_API_SCRATCH"/*) ;; *) exit 1 ;; esac
case "$EQIORA_SITE_RUSTDOC_TARGET" in "$EQIORA_API_SCRATCH"/*) ;; *) exit 1 ;; esac
case "$EQIORA_SITE_RUSTDOC_STAGE" in "$EQIORA_API_SCRATCH"/*) ;; *) exit 1 ;; esac
case "$EQIORA_SITE_ARTIFACT" in "$EQIORA_API_SCRATCH"/*) ;; *) exit 1 ;; esac
case "$PLAYWRIGHT_BROWSERS_PATH" in */eqiora-pw-1.62.1-r1234) ;; *) exit 1 ;; esac

cd "$EQIORA_SITE_SOURCE_ROOT"
test "$(git rev-parse --show-toplevel)" = "$EQIORA_SITE_SOURCE_ROOT"
test "$(git rev-parse HEAD)" = "$EQIORA_SITE_SOURCE_SHA"
test -z "$(git status --porcelain=v1 --untracked-files=all)"
test -z "$(find "$EQIORA_API_SCRATCH" -mindepth 1 -maxdepth 1 -print -quit)"

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
python3 -m unittest discover -s tools/site/tests/site -p 'test_*.py' -v

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
mkdir "$invalid_repository"
git archive --format=tar --output="$EQIORA_API_SCRATCH/invalid-math-source.tar" HEAD
tar -xf "$EQIORA_API_SCRATCH/invalid-math-source.tar" -C "$invalid_repository"
ln -s "$EQIORA_SITE_SOURCE_ROOT/docs/site/node_modules" "$invalid_site/node_modules"
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
grep -Eiq 'katex|parse error|unexpected end|expected' "$invalid_log"

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

test "$(git rev-parse HEAD)" = "$EQIORA_SITE_SOURCE_SHA"
test -z "$(git status --porcelain=v1 --untracked-files=all)"

echo "offline site checks: exact artifact, browser, and accessibility contract passed"
