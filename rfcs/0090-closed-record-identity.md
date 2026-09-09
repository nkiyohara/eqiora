# RFC 0090: Closed record identity

Status: accepted design for closed record authoring and canonical replay.

## Invariant and owner

A closed record owns an ordered, nonempty set of uniquely named members. Each
member retains its complete existing mathematical value type. A value or bus
belongs to exactly one declaration, independently of its display name, member
names, or matching numerical values. Current Model replay must preserve this
identity, reject foreign, missing, or mistyped member bindings, and retain
declaration order as part of value meaning. Swapping same-typed members changes
the value rather than making an otherwise valid record undecodable.

The homogeneous `ValueType` owner cannot represent heterogeneous dimensions and
scalar domains without inventing misleading scalar, shape, frame, and dimension
answers. A Component is elaborated source structure, and a Port is a causal or
physical endpoint; neither owns an immutable product value's nominal identity.
Flattened unrelated Fields alone erase the distinction between a record and a
coincidentally similar group of fields. An external compiler catalog would make
canonical Model admission depend on authority outside the Model.

Consequently two semantic entities own the irreducible facts: `Record` owns the
ordered declaration, and `RecordInstance` retains an existing expression DAG with one root per ordered member.
Ordinary Fields and Parameters continue to own leaf execution, differentiation,
and storage. Record instances neither add an evaluator nor masquerade as vectors.
Source and native authoring consume the same checked definitions; canonical
Model admission and replay independently validate their typed member references.

## Temporal and value boundary

A bus has one explicit activation shared by its members. Mixed-clock ownership
is rejected, and no member clock is inferred or coerced. Static record values
contain only static member expressions, preserving original Parameter dependence.
Bus roots directly select existing Fields; arithmetic on a member is an ordinary
leaf expression and does not change bus ownership. Enum, Boolean, and integer members stay
discrete; admitted real/complex leaves use the existing differentiation owner.
Declaration order is canonical even when a constructor supplies members in a
different order. Missing, duplicate, unknown, and foreign members reject before
execution. There is no nullable/open record, Python-object payload, or callback.

The delivered initial product is a flat closed record: heterogeneous numeric,
Boolean, and enum member types are admitted; recursive record declarations,
record arrays, and variant payloads are not implied. Compile-time enum choices
retain their selected equations and provenance; runtime mode changes remain
under the separate event/mode contract.

## Evidence and migration

Focused tests exercise a typed sensor/command bus and a controller-mode record,
source/native/Python agreement, exact current Model replay, temporal cross-wires,
and membership falsifiers. Existing homogeneous leaf contracts remain the
execution authority. Current source identity and Model schema epochs advance
atomically with all affected consumers; no legacy decoder or compatibility shim
is introduced. This RFC does not declare these requirements implemented merely
by specifying them.
