#!/usr/bin/env python3
"""Regenerate the bounded XDMF/HDF5 reference fixture."""

from __future__ import annotations

import argparse
from pathlib import Path

import h5py
import numpy as np


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


def write_fixture(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)

    geometry = np.asarray(
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        dtype="<f8",
    )
    topology = np.asarray([[0, 1, 2], [0, 2, 3]], dtype="<u8")
    temperature = np.asarray([10.0, 20.0, 30.0, 40.0], dtype="<f8")
    flux = np.asarray([[1.0, 0.0], [0.0, 1.0]], dtype="<f8")

    with h5py.File(output / "unit-square.h5", "w", libver="earliest") as h5:
        mesh = h5.create_group("mesh", track_order=False)
        fields = h5.create_group("fields", track_order=False)
        mesh.create_dataset("topology", data=topology, track_times=False)
        mesh.create_dataset("geometry", data=geometry, track_times=False)
        fields.create_dataset("temperature", data=temperature, track_times=False)
        fields.create_dataset("flux", data=flux, track_times=False)

    (output / "unit-square.xdmf").write_text(XDMF, encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    write_fixture(args.output)


if __name__ == "__main__":
    main()
