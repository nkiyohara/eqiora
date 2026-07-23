# Reference generation

The checked-in corpus is generated with exact Python tool releases. The h5py
wheel must report its bundled HDF5 2.0.0 runtime; the generator fails closed if
any of the three versions differs:

```bash
uv run --with h5py==3.16.0 --with numpy==2.5.1 \
  verify/artifacts/xdmf-hdf5-native-import/references/generate_fixture.py \
  verify/artifacts/xdmf-hdf5-native-import/fixtures
```

The generator creates one admitted XDMF/HDF5 pair and a hostile HDF5 corpus.
Every hostile image retains the complete admitted request set and adds one
independently recognizable, unrequested forbidden storage construct. Eqiora
must therefore audit and reject the complete image before returning any values;
a missing requested dataset cannot satisfy a falsifier accidentally. The
generator is not an oracle for Eqiora artifact identity.
