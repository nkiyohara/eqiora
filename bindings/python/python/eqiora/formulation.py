"""Mathematical Formulation requests and resolved-selection inspection."""

from ._eqiora import FormulationKind, FormulationSelectionMode, FormulationView

PrimalGalerkin = FormulationKind.PrimalGalerkin
MixedGalerkin = FormulationKind.MixedGalerkin
IntegralConservative = FormulationKind.IntegralConservative

__all__ = [
    "FormulationKind",
    "FormulationSelectionMode",
    "FormulationView",
    "PrimalGalerkin",
    "MixedGalerkin",
    "IntegralConservative",
]
