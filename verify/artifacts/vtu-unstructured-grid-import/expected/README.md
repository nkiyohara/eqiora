# Independent expected content

`summary.json` is the format-neutral expected content decoded independently
from the fixture-generation program. The triangle signed double areas are
computed in XY as

```text
(x1 - x0) (y2 - y0) - (y1 - y0) (x2 - x0)
```

and are positive for both cells. Field values are tuple-major. The exact
structural selectors are Piece `[0,0]`, point Field `[0,0,0,0]`, and cell
Field `[0,0,1,0]`. Association and `Name` are expected metadata validated
after selection, not selector identity. VTK range metadata is deliberately
absent because it is derived writer information, not accepted Field content.
The normalized geometry selector is Points DataArray `[0,0,2,0]`. The
normalized topology selector is composite Cells `[0,0,3]`, covering its
connectivity, offsets, and types together.

`source.sha256` covers the exact checked-in VTU bytes. Product-level mesh,
Field, manifest, and aggregate import digests belong to the finalized adapter
and public workflow contract; this fixture lane does not guess them.
