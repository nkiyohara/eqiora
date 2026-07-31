# Frozen observations

No image baseline or new numerical observation is frozen.

The adapter must send the accepted Result's complete coordinates,
connectivity, and P1 pressure values unchanged to the public Matplotlib
triangular renderer. Its color limits are exactly the Result's already
accepted pressure extrema, and its labels retain metre coordinates and
pressure in pascals.

The output oracle checks only that a real headless canvas draws and that the
caller can encode a valid, decodable, nonblank PNG. PNG bytes, pixels,
dimensions, compression, metadata, fonts, and layout metrics remain unfrozen.
