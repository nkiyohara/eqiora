# Single entry point for the Julia oracle route. Julia standard library only.

module Oracle

include("geometry.jl")
include("mini.jl")
include("witness.jl")
include("audit.jl")
include("falsify.jl")

using .Geometry, .Mini, .Witness, .Audit, .Falsify

end # module
