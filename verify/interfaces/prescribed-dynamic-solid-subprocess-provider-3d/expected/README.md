# Frozen exact transport and publication bytes

`candidate.bin` is the 96-byte little-endian binary64 total-displacement block.
`transcript.bin` concatenates, for each successful frame, its direction byte,
16-byte prefix, and exact payload.  Both remain non-UTF-8 binary fixtures.

The occurrence and Run files contain compact canonical JSON followed by one
repository newline.  Their artifact digests exclude that newline.  Compact
one-line layout is intentional: it makes the transition guard observe the
precommitted qualified-identity counts for these two exact fixtures.

The bytes are an independent oracle, not serialized provider output and not a
general transcript log, failure record, launch description, or migration
format.  Regeneration is permitted only from the accepted predecessor inputs
and frozen public protocol contract.
