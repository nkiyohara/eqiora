"""Typed boundary for the private authoring adapter."""

from collections.abc import Mapping
from .. import ValueType
from . import Module, Record as Record, RecordField as RecordField, RecordParameter as RecordParameter

def declare_record(source: Module, name: str, *, members: Mapping[str, ValueType], doc: str | None = None) -> Record: ...
