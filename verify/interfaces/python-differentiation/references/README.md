# References

Independent centered finite differences are produced by compiling and solving
immutable child Model revisions. No external reference dataset is required.
The analytic actions under test come from Eqiora's `LinearizedRelation` and
paired `LinearizedOutput` contracts; NumPy is used only for array comparison
and inner products at the Python boundary.
