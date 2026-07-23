# Reference provenance

The checked-in metadata follows the official XDMF model: one `Uniform` grid,
`Triangle` topology, `XY` geometry, HDF-backed `DataItem` arrays, and `Node`
and `Cell` attributes. The model and format reference is:

- <https://www.xdmf.org/index.php/XDMF_Model_and_Format>

The HDF5 file is an independent, complete source-byte fixture rather than an
Eqiora serialization. Dataset shape, primitive type, and value can be checked
with the official HDF5 `h5dump` reader independently of the Eqiora adapter:

```bash
h5dump -H ../fixtures/unit-square.h5
h5dump -d /mesh/topology ../fixtures/unit-square.h5
h5dump -d /mesh/geometry ../fixtures/unit-square.h5
h5dump -d /fields/temperature ../fixtures/unit-square.h5
h5dump -d /fields/flux ../fixtures/unit-square.h5
```

The relevant HDF5 documentation is:

- <https://support.hdfgroup.org/documentation/hdf5/latest/_h5_d__u_g.html>

The fixture was generated on CPython 3.13.14 by `h5py 3.16.0`, linked to
HDF5 2.0.0, with NumPy 2.5.1. Regenerate it in an isolated environment:

```bash
tmp=$(mktemp -d)
uv python install 3.13 --install-dir "$tmp/python" --no-bin
python_bin=$(find "$tmp/python" -type f -path '*/bin/python3.13' | head -1)
uv venv --python "$python_bin" "$tmp/venv"
uv pip install --python "$tmp/venv/bin/python" \
  'h5py==3.16.0' 'meshio==5.3.5'
"$tmp/venv/bin/python" generate_fixture.py ../fixtures
sha256sum ../fixtures/unit-square.xdmf ../fixtures/unit-square.h5
```

As a second independent reader, meshio 5.3.5 can load the XDMF/HDF5 pair and
must report four points, two triangles, node scalar `temperature`, and
cell vector `flux`:

```bash
"$tmp/venv/bin/python" - <<'PY'
import meshio

mesh = meshio.read("../fixtures/unit-square.xdmf")
print(mesh.points)
print(mesh.cells_dict["triangle"])
print(mesh.point_data["temperature"])
print(mesh.cell_data_dict["flux"]["triangle"])
PY
```

XDMF and HDF5 are independent input oracles only. Eqiora's accepted authority
is the typed plan replayed through the shared mesh, field, provenance, and
artifact invariants; source paths and the generating tool versions are not
semantic identity.
