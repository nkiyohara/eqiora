#!/usr/bin/env python3
"""Independent derivation of the frozen square-with-hole geometry identity.

Written only from RFC 0079: wire field order and kebab-case names,
canonicalization rules, and sha256(schema || 0x00 || canonical bytes).
"""

import hashlib
import json

SCHEMA = "eqiora.geometry-definition-envelope/v1"
ENCODING = "eqiora.canonical-json/v1"
TOLERANCE_M = 0.0625
EXPECTED_BYTES = 482
EXPECTED_SHA256 = "e6f8e17ac215ef37ca3c9de07b9979e34f13412a5de11dc9240ea1def8130030"
EXPECTED_JSON = (
    '{"schema":"eqiora.geometry-definition-envelope/v1",'
    '"encoding":"eqiora.canonical-json/v1",'
    '"kind":"straight-edged-planar-v1","length-unit":"metre",'
    '"tolerance-m":0.0625,'
    '"vertices":[[0.0,0.0],[0.0,1.0],[0.25,0.25],[0.25,0.75],'
    '[0.75,0.25],[0.75,0.75],[1.0,0.0],[1.0,1.0]],'
    '"faces":[{"outer":[0,6,7,1],"holes":[[2,3,5,4]]}],'
    '"entity-sets":[{"name":"exterior","dimension":1,"members":[0,1,2,3]},'
    '{"name":"hole","dimension":1,"members":[4,5,6,7]},'
    '{"name":"fluid","dimension":2,"members":[0]}]}'
)

OUTER = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
HOLE = [(0.25, 0.25), (0.75, 0.25), (0.75, 0.75), (0.25, 0.75)]


def signed_area(loop, vertices):
    area = 0.0
    for position, vertex in enumerate(loop):
        x1, y1 = vertices[vertex]
        x2, y2 = vertices[loop[(position + 1) % len(loop)]]
        area += x1 * y2 - x2 * y1
    return area / 2.0


def canonical_loop(loop, vertices, want_ccw):
    if (signed_area(loop, vertices) > 0.0) != want_ccw:
        loop = list(reversed(loop))
    start = loop.index(min(loop))
    return loop[start:] + loop[:start]


authored = OUTER + HOLE
order = sorted(range(len(authored)), key=lambda index: authored[index])
vertices = [list(authored[index]) for index in order]
remap = {old: new for new, old in enumerate(order)}

outer = canonical_loop([remap[index] for index in range(4)], vertices, True)
hole = canonical_loop([remap[index] for index in range(4, 8)], vertices, False)
faces = [{"outer": outer, "holes": sorted([hole])}]
faces.sort(key=lambda face: face["outer"])

entity_sets = sorted(
    [
        {"name": "exterior", "dimension": 1, "members": [0, 1, 2, 3]},
        {"name": "fluid", "dimension": 2, "members": [0]},
        {"name": "hole", "dimension": 1, "members": [4, 5, 6, 7]},
    ],
    key=lambda entity_set: (entity_set["dimension"], entity_set["name"]),
)

wire = {
    "schema": SCHEMA,
    "encoding": ENCODING,
    "kind": "straight-edged-planar-v1",
    "length-unit": "metre",
    "tolerance-m": TOLERANCE_M,
    "vertices": vertices,
    "faces": faces,
    "entity-sets": entity_sets,
}

text = json.dumps(
    wire, separators=(",", ":"), ensure_ascii=False, allow_nan=False
)
data = text.encode("utf-8")
digest = hashlib.sha256(SCHEMA.encode("utf-8") + b"\x00" + data).hexdigest()

assert text == EXPECTED_JSON
assert len(data) == EXPECTED_BYTES
assert digest == EXPECTED_SHA256
assert hashlib.sha256(data).hexdigest() != EXPECTED_SHA256
assert (
    hashlib.sha256(SCHEMA.encode("utf-8") + data).hexdigest()
    != EXPECTED_SHA256
)

print(text)
print("bytes:", len(data))
print("sha256:", digest)
