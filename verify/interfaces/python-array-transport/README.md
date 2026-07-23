# Python immutable CPU array transport

This case verifies one bounded Result-producer data plane. A successful
semantic-reference run transfers each owned one-dimensional native `f64`
buffer into an opaque `Array` without copying. Inspecting the descriptor does
not import NumPy.

The first NumPy projection owns the transferred allocation through a private
base object. `copy=False` and `copy=None` return the same C-contiguous, aligned,
read-only ndarray; Python cannot restore write access, and the view survives
the originating Result. `copy=True` returns an independent writable copy.

DLPack is deliberately different. Its read-only flag is advisory for
consumers, so every Eqiora `Array.__dlpack__` export is an independent
versioned 1.x CPU snapshot. `copy=False`, a legacy capsule request, a
non-`None` stream, or a non-CPU device request fails with `BufferError`.
NumPy supplies the versioned capsule, flags, and deleter; the registered test
falsifies non-aliasing, request rejection, and single consumption rather than
adding a second unsafe capsule implementation.

This producer case does not admit array inputs, portable-Realization outputs,
GPU, distributed or sparse arrays, legacy DLPack, framework-wide zero-copy, or
memory isolation from callers that re-export the NumPy view or use native
pointer access. The separate
[`interfaces.python-differentiation`](../python-differentiation/README.md) case
owns the bounded CPU DLPack input consumer; wider consumers require their own
evidence.

Run the registered evidence:

```bash
cargo test --locked -p eqiora-python --test python_array_transport
cargo run --locked -p eqiora-verify -- run --case interfaces.python-array-transport
```

The installed-wheel companion is
`bindings/python/tests/test_array_transport.py`.
