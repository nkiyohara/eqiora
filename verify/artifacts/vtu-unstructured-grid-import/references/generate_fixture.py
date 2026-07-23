"""Regenerate the bounded ASCII VTU fixture with official VTK Python 9.5.2."""

from pathlib import Path

import vtk


EXPECTED_VTK_VERSION = "9.5.2"
OUTPUT = Path(__file__).resolve().parents[1] / "fixtures" / "unit-square-tri3-ascii.vtu"


def main() -> None:
    actual_version = vtk.vtkVersion.GetVTKVersion()
    if actual_version != EXPECTED_VTK_VERSION:
        raise RuntimeError(
            f"fixture requires VTK {EXPECTED_VTK_VERSION}, found {actual_version}"
        )

    points = vtk.vtkPoints()
    points.SetDataTypeToDouble()
    for point in (
        (0.0, 0.0, 0.0),
        (1.0, 0.0, 0.0),
        (1.0, 1.0, 0.0),
        (0.0, 1.0, 0.0),
    ):
        points.InsertNextPoint(*point)

    grid = vtk.vtkUnstructuredGrid()
    grid.SetPoints(points)
    for point_ids in ((0, 1, 2), (0, 2, 3)):
        triangle = vtk.vtkTriangle()
        for local_id, point_id in enumerate(point_ids):
            triangle.GetPointIds().SetId(local_id, point_id)
        grid.InsertNextCell(triangle.GetCellType(), triangle.GetPointIds())

    temperature = vtk.vtkDoubleArray()
    temperature.SetName("temperature")
    temperature.SetNumberOfComponents(1)
    for value in (300.0, 310.0, 320.0, 330.0):
        temperature.InsertNextValue(value)
    grid.GetPointData().SetScalars(temperature)

    flux = vtk.vtkDoubleArray()
    flux.SetName("flux")
    flux.SetNumberOfComponents(2)
    for value in ((1.0, 0.0), (0.0, 1.0)):
        flux.InsertNextTuple(value)
    grid.GetCellData().AddArray(flux)

    writer = vtk.vtkXMLUnstructuredGridWriter()
    writer.SetFileName(str(OUTPUT))
    writer.SetInputData(grid)
    writer.SetDataModeToAscii()
    writer.SetCompressorTypeToNone()
    writer.SetWriteTimeValue(False)
    if writer.Write() != 1:
        raise RuntimeError("VTK failed to write the fixture")


if __name__ == "__main__":
    main()
