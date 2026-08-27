# Expected result

The gate accepts exactly one source distribution and four wheels. The emitted
manifest has format `eqiora.python-distribution-candidate/v3`, marks the full
profile set complete, records a clean source commit, and contains five
artifact hashes. The complete profile set includes every public base quick
start before upload, the CPython 3.13 framework quick starts, an independently
installed CPython 3.12 profile whose recorded NumPy version is exactly 2.1.0,
and the exact CPython 3.13 `notebook` profile. That profile runs the installed
exact-cylinder Marimo application and its caller-owned Matplotlib Figure; it
does not install or validate a private Eqiora notebook viewer. The manifest
contains the exact host checks and the closed Node 24.18.1/npm 11.16.0,
Marimo 0.23.16, Playwright 1.62.1, and managed Chromium 1234/151.0.7922.34
identity.

The gate accepts the v3 family only with the exact canonical detached H2
receipt bound to its source commit, complete artifact inventory, structured
inventory preimages, browser, and Python host. Both clean frontend-host
validations consume identical locked inputs and pass type/lint checks without
post-install external requests. A
genuinely signal-free historical v2 family remains readable; any N1 signal in
the sdist, a wheel, manifest, or requested profile instead activates v3 and
fails closed if the family is incomplete.

The accepted execution order is exact-revision `prepare` to the sole H2
executor to `finalize`. The first stage leaves only the one-sdist/four-wheel
family, H2 reads frontend authority only from its safe-extracted sdist and uses
two fully disjoint home-backed build roots, and finalization consumes without
rebuilding or manufacturing a receipt. The family inventory is identical on
entry and exit at every later stage. Only the family is publishable; the
canonical receipt and completed manifest remain separate metadata.

No generated distribution artifact, product frontend asset, lockfile, or H2
PASS receipt is supplied by this expected-result directory. A retained
TestPyPI or PyPI candidate belongs to the release channel and remains bound to
its candidate manifest and companion H2 receipt.
