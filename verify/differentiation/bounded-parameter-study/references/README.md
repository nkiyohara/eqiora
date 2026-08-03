# Reference strategy

The reference is relational rather than numerical. Before study execution,
the public integration test invokes the existing accepted
`DifferentiableProgram::evaluate` independently for the exact canonical
complete points with diffusion values `0.75`, `1.0`, and `1.25`. Each study
member must reproduce the corresponding immutable evaluation's exact public
observations.

The private child-module tests use the frozen `execute_with_evaluator` and
`CompleteParameterStudy::from_members` seams solely to inject composition
mutants. They add no executor abstraction and publish no member constructor.
