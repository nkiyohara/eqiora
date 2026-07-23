# Reference provenance

The reference is the validated in-memory `ResolvedRealization` produced from
the compiled Semantic Model and exact capability set. Private wire DTOs are
decoded back through the public typed constructors, then compared with that
reference and with the linked model and run artifacts.
