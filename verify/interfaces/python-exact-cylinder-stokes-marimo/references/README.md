# Reference provenance

This application evidence was authored without reading an implementation of
the new Marimo app. It composes these already accepted owners without changing
or copying their expected values:

- `interfaces.python-exact-circular-hole-geometry` for Geometry;
- `interfaces.python-circular-hole-chordal-mesh` for the typed provider, MeshPlan,
  Mesh, exact-source binding, and realized selection meaning;
- `interfaces.python-exact-cylinder-stokes-result` for the installed Model,
  typed Plan, one Run, common Result, pressure Field, and typed evidence;
- `interfaces.python-exact-cylinder-pressure-still` for the caller-owned
  pressure Figure; and
- `fluid.exact-circular-hole-stokes-2d-gmsh` for all scientific observations and
  tolerances.

Marimo 0.23.16 and the existing CPython 3.13 candidate profile own host
reachability. The browser oracle observes DOM state rather than scientific
pixels.

[`exact_cylinder_stokes_marimo_repository_helper_mutant.py`](exact_cylinder_stokes_marimo_repository_helper_mutant.py)
is the one precommitted wrong application. It imports the repository
command-line example helper at top level. The candidate executes it as a
plain dependency probe with no `app.run()` entry point, only after the live
positive. Its clean consumer contains no helper or other member, so the
top-level import fails before a Marimo cell or Eqiora Run can execute. The
oracle accepts only that exact missing-`examples` failure.
