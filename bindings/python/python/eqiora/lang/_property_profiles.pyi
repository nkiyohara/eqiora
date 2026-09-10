"""Typed boundary for the private authoring adapter."""

from collections.abc import Mapping
from dataclasses import dataclass
from .. import ValueType
from . import Expression, Module, PropertyContract, PropertyRequirement

@dataclass(frozen=True, slots=True)
class ContractProfile:
    inputs: tuple[tuple[str, ValueType], ...]
    derivatives: str
    branch: str | None

@dataclass(frozen=True, slots=True)
class ReleaseProfile:
    validity: Expression | None
    branch: str | None
    outside: str

def contract_profile(module: Module, inputs: Mapping[str, ValueType] | None, derivatives: str, branch: str | None) -> ContractProfile: ...
def formal(contract: PropertyContract, name: str) -> Expression: ...
def release_profile(contract: PropertyContract, value: object, validity: object, branch: str | None, outside: str) -> tuple[Expression, ReleaseProfile]: ...
def apply(requirement: PropertyRequirement, arguments: dict[str, object]) -> Expression: ...
