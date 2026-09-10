"""Typed boundary for the private authoring adapter."""

from . import Component, Expression, Relation, Support

def declare(component: Component, name: str, on: Support, flux: Expression, source: Expression, doc: str | None) -> Relation: ...
