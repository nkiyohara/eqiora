# Expected evidence

There is no producer-generated golden file and no numerical tolerance. The
test owns exact expected Model literals, route-sealed coordinate bit patterns,
nominal spacing and face-area bit patterns, inventory counts, and an independent
complete packet-law replay from the accepted evidence contract.

The collocated mesh digest is intentionally not a literal. The test obtains it
from the ordinary `CartesianMeshEnvelopeV1` producer applied to the sealed hex
coordinates, compares the returned view with that producer identity, and
requires it to differ from the accepted `2 x 3 x 4` predecessor digest.

Axis 1 has three distinct consecutive binary64 widths even though the accepted
nominal spacing is one exact class constant. The case therefore checks the
view's nominal spacing bits while explicitly making no claim that this value is
the mesh-byte lifted seam distance.
