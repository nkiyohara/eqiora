"""Canonical JSON serialization for the immutable mesh copy.

The rule is fixed here so the mesh artifact has one spelling and one SHA-256.
It is deliberately small and closed:

- UTF-8, LF line endings, exactly one trailing newline;
- object members are emitted in the order the schema fixes (never sorted), each
  on its own line, indented by two spaces per level;
- an array whose elements are all scalars, or all arrays of scalars, is emitted
  inline with ``,`` separators and no spaces; every other array puts one element
  per line;
- ``float`` uses ``repr``, which is CPython's shortest round-trip binary64
  spelling and therefore keeps ``.0`` on integral values and the lowercase
  exponent form; negative zero normalizes to positive zero;
- ``bool`` is ``true``/``false``, ``None`` is ``null``, ``int`` is decimal, and
  strings use ``json.dumps(..., ensure_ascii=True)``;
- non-finite floats are rejected.

This module has no dependency outside the standard library.
"""

from __future__ import annotations

import json
import math

__all__ = ["dumps", "dump_bytes"]


def _scalar(value: object) -> str:
    if value is None:
        return "null"
    if value is True:
        return "true"
    if value is False:
        return "false"
    if isinstance(value, int):
        return repr(value)
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ValueError(f"non-finite float is not canonical: {value!r}")
        if value == 0.0:
            value = 0.0  # normalize -0.0
        return repr(value)
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=True)
    raise TypeError(f"unsupported canonical JSON scalar: {type(value).__name__}")


def _is_scalar(value: object) -> bool:
    return value is None or isinstance(value, (bool, int, float, str))


def _inline_array(value: list) -> bool:
    if all(_is_scalar(item) for item in value):
        return True
    return all(
        isinstance(item, list) and all(_is_scalar(x) for x in item) for item in value
    )


def _render(value: object, indent: int) -> str:
    pad = " " * indent
    inner = " " * (indent + 2)
    if isinstance(value, dict):
        if not value:
            return "{}"
        body = ",\n".join(
            f"{inner}{json.dumps(str(k), ensure_ascii=True)}: {_render(v, indent + 2)}"
            for k, v in value.items()
        )
        return "{\n" + body + "\n" + pad + "}"
    if isinstance(value, list):
        if not value:
            return "[]"
        if _inline_array(value):
            parts = []
            for item in value:
                if isinstance(item, list):
                    parts.append("[" + ",".join(_scalar(x) for x in item) + "]")
                else:
                    parts.append(_scalar(item))
            return "[" + ",".join(parts) + "]"
        body = ",\n".join(f"{inner}{_render(item, indent + 2)}" for item in value)
        return "[\n" + body + "\n" + pad + "]"
    return _scalar(value)


def dumps(document: object) -> str:
    """Return the canonical text, including its single trailing newline."""
    return _render(document, 0) + "\n"


def dump_bytes(document: object) -> bytes:
    """Return the canonical UTF-8 bytes, including the trailing newline."""
    return dumps(document).encode("utf-8")
