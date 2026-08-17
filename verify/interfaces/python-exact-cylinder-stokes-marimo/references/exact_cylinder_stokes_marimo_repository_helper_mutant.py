"""Wrong app probe: depend on the repository command-line example helper."""

from examples.python.exact_cylinder_stokes import solve as repository_solve
import marimo


__generated_with__ = "0.23.16"
app = marimo.App(width="medium")


print("EQIORA_REPOSITORY_HELPER_UNEXPECTEDLY_RESOLVED_BEFORE_RUN", flush=True)


@app.cell
def _():
    result = repository_solve()
    return (result,)


# There is deliberately no app.run() entry point. The candidate executes this
# source only as a dependency-resolution falsifier; if the helper unexpectedly
# resolves, the process exits without executing a Marimo cell or Eqiora Run.
