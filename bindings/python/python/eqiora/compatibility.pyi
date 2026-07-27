from typing import ClassVar, final

from . import Model, _ModelDeclaration

@final
class ExactModelCodec:
    V1: ClassVar[ExactModelCodec]
    V2: ClassVar[ExactModelCodec]
    V3: ClassVar[ExactModelCodec]
    V4: ClassVar[ExactModelCodec]
    V5: ClassVar[ExactModelCodec]
    V6: ClassVar[ExactModelCodec]
    V7: ClassVar[ExactModelCodec]
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

def compile_exact(
    source: str,
    *,
    filename: str = "<memory>",
    codec: ExactModelCodec,
) -> Model: ...

def define_exact(
    name: str,
    *declarations: _ModelDeclaration,
    codec: ExactModelCodec,
) -> Model: ...
def replay_exact(data: bytes, *, codec: ExactModelCodec) -> Model: ...

__all__ = ["ExactModelCodec", "compile_exact", "define_exact", "replay_exact"]
