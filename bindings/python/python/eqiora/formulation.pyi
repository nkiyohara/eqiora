"""Mathematical Formulation requests and resolved-selection inspection.

Authority: ``bindings/python/python/eqiora/formulation.py``.
"""
from typing import Final

from . import FormulationKind, FormulationSelectionMode, FormulationView

#: Primal Galerkin form used by scalar and displacement fields.
#:
#: Authority: ``crates/eqiora-python/src/common_plan/capability_view.rs::PyFormulationKind``.
PrimalGalerkin: Final[FormulationKind]

#: Mixed Galerkin form used by coupled velocity-pressure fields.
#:
#: Authority: ``crates/eqiora-python/src/common_plan/capability_view.rs::PyFormulationKind``.
MixedGalerkin: Final[FormulationKind]

#: Integral-conservative form used by face-flux finite volumes.
#:
#: Authority: ``crates/eqiora-python/src/common_plan/capability_view.rs::PyFormulationKind``.
IntegralConservative: Final[FormulationKind]

__all__ = [
    "FormulationKind",
    "FormulationSelectionMode",
    "FormulationView",
    "PrimalGalerkin",
    "MixedGalerkin",
    "IntegralConservative",
]
