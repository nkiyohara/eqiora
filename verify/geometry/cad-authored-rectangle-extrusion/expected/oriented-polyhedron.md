# Independent oriented-polyhedron oracle

Let the lower vertices be `A=(-2,-1,1/2)`, `B=(3,-1,1/2)`,
`C=(3,2,1/2)`, and `D=(-2,2,1/2)`, with primed vertices translated to
`z=9/2`.  The outward cycles are:

1. `A,D,C,B`;
2. `A',B',C',D'`;
3. `A,A',D',D`;
4. `B,C,C',B'`;
5. `A,B,B',A'`; and
6. `D,D',C',C`.

They contain 8 vertices, 12 edges used twice with opposite orientation, and 6
faces, so `V-E+F=2` and the area-vector sum is zero.  Splitting each quadrilateral
into two outward triangles gives signed determinant contributions
`-15, 135, 48, 72, 40, 80 m^3`; division of their sum by six gives exactly
`60 m^3`.  The six areas sum to `15+15+12+12+20+20 = 94 m^2`.
