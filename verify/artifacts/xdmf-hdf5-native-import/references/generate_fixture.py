#!/usr/bin/env python3
"""Regenerate the native HDF5 file-image acceptance and rejection corpus."""

from __future__ import annotations

import argparse
import ctypes
import os
from pathlib import Path

import h5py
import numpy as np


EXPECTED_H5PY = "3.16.0"
EXPECTED_HDF5 = "2.0.0"
EXPECTED_NUMPY = "2.5.1"


XDMF = """<?xml version="1.0" encoding="UTF-8"?>
<Xdmf Version="3.0">
  <Domain>
    <Grid Name="unit-square" GridType="Uniform">
      <Topology TopologyType="Triangle" NumberOfElements="2">
        <DataItem Format="HDF" DataType="UInt" Precision="8" Dimensions="2 3">unit-square.h5:/mesh/topology</DataItem>
      </Topology>
      <Geometry GeometryType="XY">
        <DataItem Format="HDF" DataType="Float" Precision="8" Dimensions="4 2">unit-square.h5:/mesh/geometry</DataItem>
      </Geometry>
      <Attribute Name="temperature" AttributeType="Scalar" Center="Node">
        <DataItem Format="HDF" DataType="Float" Precision="8" Dimensions="4">unit-square.h5:/fields/temperature</DataItem>
      </Attribute>
      <Attribute Name="flux" AttributeType="Vector" Center="Cell">
        <DataItem Format="HDF" DataType="Float" Precision="8" Dimensions="2 2">unit-square.h5:/fields/flux</DataItem>
      </Attribute>
    </Grid>
  </Domain>
</Xdmf>
"""


def arrays() -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    geometry = np.asarray(
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        dtype="<f8",
    )
    topology = np.asarray([[0, 1, 2], [0, 2, 3]], dtype="<u8")
    temperature = np.asarray([10.0, 20.0, 30.0, 40.0], dtype="<f8")
    flux = np.asarray([[1.0, 0.0], [0.0, 1.0]], dtype="<f8")
    return geometry, topology, temperature, flux


def populate_admitted(h5: h5py.File) -> None:
    geometry, topology, temperature, flux = arrays()
    mesh = h5.create_group("mesh", track_order=False)
    fields = h5.create_group("fields", track_order=False)
    mesh.create_dataset("topology", data=topology, track_times=False)
    mesh.create_dataset("geometry", data=geometry, track_times=False)
    fields.create_dataset("temperature", data=temperature, track_times=False)
    fields.create_dataset("flux", data=flux, track_times=False)


def write_admitted(path: Path) -> None:
    with h5py.File(path, "w", libver="earliest") as h5:
        populate_admitted(h5)


def write_soft_link(path: Path) -> None:
    with h5py.File(path, "w", libver="earliest") as h5:
        populate_admitted(h5)
        h5["forbidden"] = h5py.SoftLink("/mesh/geometry")


def write_external_link(path: Path) -> None:
    with h5py.File(path, "w", libver="earliest") as h5:
        populate_admitted(h5)
        h5["forbidden"] = h5py.ExternalLink("never-open.h5", "/value")


def write_filtered(path: Path) -> None:
    with h5py.File(path, "w", libver="earliest") as h5:
        populate_admitted(h5)
        h5.create_dataset(
            "forbidden",
            data=np.asarray([1.0, 2.0], dtype="<f8"),
            compression="gzip",
            track_times=False,
        )


def write_external_storage(path: Path) -> None:
    output = path.parent.resolve()
    previous = Path.cwd()
    raw_name = path.with_suffix(".raw").name
    try:
        os.chdir(output)
        with h5py.File(path.name, "w", libver="earliest") as h5:
            populate_admitted(h5)
            dataset = h5.create_dataset(
                "forbidden",
                shape=(2,),
                dtype="<f8",
                external=[(raw_name, 0, h5py.h5f.UNLIMITED)],
                track_times=False,
            )
            dataset[...] = np.asarray([1.0, 2.0], dtype="<f8")
        Path(raw_name).unlink()
    finally:
        os.chdir(previous)


def write_virtual(path: Path) -> None:
    with h5py.File(path, "w", libver="latest") as h5:
        populate_admitted(h5)
        h5.create_dataset(
            "forbidden-source",
            data=np.asarray([1.0, 2.0], dtype="<f8"),
            track_times=False,
        )
        space = h5py.h5s.create_simple((2,))
        creation = h5py.h5p.create(h5py.h5p.DATASET_CREATE)
        creation.set_obj_track_times(False)
        creation.set_virtual(space, path.name.encode(), b"/forbidden-source", space)
        datatype = h5py.h5t.py_create(np.dtype("<f8"))
        h5py.h5d.create(h5.id, b"forbidden", datatype, space, dcpl=creation)


def write_compound(path: Path) -> None:
    values = np.asarray([(1.0, 2.0)], dtype=[("x", "<f8"), ("y", "<f8")])
    with h5py.File(path, "w", libver="earliest") as h5:
        populate_admitted(h5)
        h5.create_dataset("forbidden", data=values, track_times=False)


def write_attribute(path: Path) -> None:
    with h5py.File(path, "w", libver="earliest") as h5:
        populate_admitted(h5)
        dataset = h5.create_dataset(
            "forbidden", data=np.asarray([1.0], dtype="<f8"), track_times=False
        )
        dataset.attrs["forbidden"] = "metadata outside the closed profile"


def write_hard_link_alias(path: Path) -> None:
    with h5py.File(path, "w", libver="earliest") as h5:
        populate_admitted(h5)
        h5["forbidden"] = h5["/mesh/geometry"]


def write_hard_link_cycle(path: Path) -> None:
    with h5py.File(path, "w", libver="earliest") as h5:
        populate_admitted(h5)
        group = h5.create_group("forbidden", track_order=False)
        group["self"] = group


def commit_datatype_without_times(
    location: h5py.h5f.FileID,
    name: bytes,
    datatype: h5py.h5t.TypeID,
) -> None:
    library = ctypes.CDLL(h5py.h5.__file__)
    hid = ctypes.c_int64
    library.H5Pcreate.argtypes = [hid]
    library.H5Pcreate.restype = hid
    library.H5Pset_obj_track_times.argtypes = [hid, ctypes.c_uint]
    library.H5Pset_obj_track_times.restype = ctypes.c_int
    library.H5Tcommit2.argtypes = [hid, ctypes.c_char_p, hid, hid, hid, hid]
    library.H5Tcommit2.restype = ctypes.c_int
    library.H5Pclose.argtypes = [hid]
    library.H5Pclose.restype = ctypes.c_int

    datatype_create_class = hid.in_dll(
        library, "H5P_CLS_DATATYPE_CREATE_ID_g"
    ).value
    creation = library.H5Pcreate(datatype_create_class)
    if creation < 0:
        raise RuntimeError("cannot create deterministic datatype property list")
    try:
        if library.H5Pset_obj_track_times(creation, 0) < 0:
            raise RuntimeError("cannot disable committed-datatype timestamps")
        if library.H5Tcommit2(location.id, name, datatype.id, 0, creation, 0) < 0:
            raise RuntimeError("cannot commit deterministic datatype")
    finally:
        if library.H5Pclose(creation) < 0:
            raise RuntimeError("cannot close datatype property list")


def write_unlinked_committed_datatype(path: Path) -> None:
    with h5py.File(path, "w", libver="earliest") as h5:
        populate_admitted(h5)
        datatype = h5py.h5t.py_create(np.dtype("<f8"))
        commit_datatype_without_times(h5.id, b"temporary-type", datatype)
        space = h5py.h5s.create_simple((2,))
        dataset_creation = h5py.h5p.create(h5py.h5p.DATASET_CREATE)
        dataset_creation.set_obj_track_times(False)
        dataset = h5py.h5d.create(
            h5.id,
            b"forbidden",
            datatype,
            space,
            dcpl=dataset_creation,
        )
        dataset.write(
            h5py.h5s.ALL,
            h5py.h5s.ALL,
            np.asarray([1.0, 2.0], dtype="<f8"),
        )
        del h5["temporary-type"]


def write_fixture(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    write_admitted(output / "unit-square.h5")
    (output / "unit-square.xdmf").write_text(XDMF, encoding="utf-8")
    write_soft_link(output / "reject-soft-link.h5")
    write_external_link(output / "reject-external-link.h5")
    write_filtered(output / "reject-filter.h5")
    write_external_storage(output / "reject-external-storage.h5")
    write_virtual(output / "reject-virtual-dataset.h5")
    write_compound(output / "reject-compound-datatype.h5")
    write_attribute(output / "reject-attribute.h5")
    write_unlinked_committed_datatype(output / "reject-unlinked-committed-datatype.h5")
    write_hard_link_alias(output / "reject-hard-link-alias.h5")
    write_hard_link_cycle(output / "reject-hard-link-cycle.h5")


def main() -> None:
    observed = (h5py.__version__, h5py.version.hdf5_version, np.__version__)
    expected = (EXPECTED_H5PY, EXPECTED_HDF5, EXPECTED_NUMPY)
    if observed != expected:
        raise RuntimeError(
            f"fixture toolchain differs: observed {observed}, expected {expected}"
        )
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    write_fixture(args.output)


if __name__ == "__main__":
    main()
