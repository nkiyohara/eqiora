# Expected result

The gate accepts exactly one source distribution and four wheels. The emitted
manifest has format `eqiora.python-distribution-candidate/v2`, marks the full
profile set complete, records a clean source commit, and contains five
artifact hashes. The complete profile set includes every public base quick
start before upload, the CPython 3.13 framework quick starts, and an
independently installed CPython 3.12 profile whose recorded NumPy version is
exactly 2.1.0.

No generated distribution artifact is committed. A retained TestPyPI or PyPI
candidate belongs to the release channel and remains bound to its candidate
manifest.
