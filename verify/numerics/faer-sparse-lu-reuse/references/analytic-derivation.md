# Exact analytic oracle for sparse-LU factorization reuse

## Scope and derivation design

The claim is the exact Q1 finite-element reduction of

\[
-\frac{d}{dx}\left(k\frac{du}{dx}\right)-s=0
\quad\text{on }[0,1],
\qquad u(0)=u(1)=b,
\]

on two uniform elements with two-point Gauss--Legendre quadrature, where the
parameter tuple is exactly `(source_scale, diffusion, boundary_offset)` =
\((s,k,b)\).

The derivation alternatives were assessed before freezing the oracle:

| View | Mathematical naturalness | Cost and complexity | Faithfulness and exactness | Use here |
| --- | --- | --- | --- | --- |
| Physical-coordinate element integration, then COO assembly and strong elimination | Native to Q1 assembly | Small; exposes every coefficient and index | Directly matches the stated discretization; exact rational arithmetic | Selected derivation |
| Direct integration of the three global hat functions | Equally exact | Slightly shorter, but hides the local arrays and duplicate COO contribution | Good cross-check, less faithful to the requested local route | Cross-check only |
| Stationarity of the discrete energy | Natural for this symmetric positive-definite reduction | Adds an auxiliary variational view and does not expose CSR ordering | Good sign and definiteness cross-check | Cross-check only |

No approximation is introduced in the selected derivation. The spatial
parameters are constant, the ordered nodes are \(x_0=0,x_1=1/2,x_2=1\), and
canonical CSR means row-major rows with sorted, unique structural column
indices. The required singular mutant explicitly stores its zero diagonal;
its canonicality is therefore understood in this structural sense. Binary64
observations below assume round-to-nearest, ties-to-even.

## Weak form and physical-coordinate element arrays

Let

\[
V_b=\{w\in H^1(0,1):w(0)=w(1)=b\},\qquad
V_0=\{v\in H^1(0,1):v(0)=v(1)=0\}.
\]

Multiplication by \(v\in V_0\), integration, and integration by parts give

\[
\int_0^1 k u'v'\,dx-\int_0^1 s v\,dx=0,
\]

because the endpoint term vanishes for the test space. Thus the algebraic
sign is

\[
K U=f,
\qquad
K_{ij}=\int_0^1 k N_i'N_j'\,dx,
\qquad
f_i=\int_0^1 sN_i\,dx.
\]

On either physical element \([a,a+h]\), with \(h=1/2\), use

\[
N_L(x)=\frac{a+h-x}{h},\qquad
N_R(x)=\frac{x-a}{h},\qquad
N_L'=-\frac1h,\quad N_R'=\frac1h.
\]

Direct physical-coordinate integration gives

\[
K^{(e)}
=\int_a^{a+h}k
\begin{bmatrix}N_L'\\N_R'\end{bmatrix}
\begin{bmatrix}N_L'&N_R'\end{bmatrix}dx
=\frac{k}{h}
\begin{bmatrix}1&-1\\-1&1\end{bmatrix}
=2k\begin{bmatrix}1&-1\\-1&1\end{bmatrix},
\]

and

\[
f^{(e)}
=\int_a^{a+h}s\begin{bmatrix}N_L\\N_R\end{bmatrix}dx
=\frac{sh}{2}\begin{bmatrix}1\\1\end{bmatrix}
=\frac{s}{4}\begin{bmatrix}1\\1\end{bmatrix}.
\]

This establishes the arrays without invoking quadrature. Separately, under
\(x=a+h(1+\xi)/2\), the stiffness integrand times the constant Jacobian has
degree zero in \(\xi\), while each load integrand has degree one. The
two-point Gauss--Legendre rule at \(\xi=\pm1/\sqrt3\), with unit weights, is
exact through degree three. In particular, its two weights sum constants to
the exact zeroth moment and symmetry cancels the linear moment. It therefore
reproduces both physical-coordinate integrals exactly; no floating evaluation
of the irrational quadrature nodes is needed for this oracle.

## Exact COO assembly, canonical CSR, and load

The uncoalesced element contributions, in element-local row-major order, are

\[
\begin{array}{c|rrrr}
e=0&(0,0,2k)&(0,1,-2k)&(1,0,-2k)&(1,1,2k)\\
e=1&(1,1,2k)&(1,2,-2k)&(2,1,-2k)&(2,2,2k).
\end{array}
\]

Coalescing the duplicate \((1,1)\) entry gives the full row-major COO list

\[
(0,0,2k),(0,1,-2k),(1,0,-2k),(1,1,4k),
(1,2,-2k),(2,1,-2k),(2,2,2k).
\]

Equivalently,

\[
K=\begin{bmatrix}
2k&-2k&0\\
-2k&4k&-2k\\
0&-2k&2k
\end{bmatrix},
\qquad
f=\begin{bmatrix}s/4\\s/2\\s/4\end{bmatrix}.
\]

Its canonical CSR structure is

```text
offsets = [0, 2, 5, 7]
columns = [0, 1, 0, 1, 2, 1, 2]
values  = [2k, -2k, -2k, 4k, -2k, -2k, 2k]
```

For \(p_0\) and \(p_1\), the values are
`[2,-2,-2,4,-2,-2,2]`; for \(p_2\), they are
`[5/2,-5/2,-5/2,5,-5/2,-5/2,5/2]`. The respective full
right-hand sides are `[1/2,1,1/2]`, `[1,2,1]`, and
`[1/2,1,1/2]`.

## Strong endpoint elimination

Set the endpoint degrees of freedom to \(U_B=[b,b]^T\) and retain only the
interior degree of freedom \(u=U_1\). The interior block and its two boundary
couplings are

\[
K_{II}=[4k],\qquad K_{IB}=\begin{bmatrix}-2k&-2k\end{bmatrix}.
\]

Strong elimination restricts the system and shifts both known boundary terms:

\[
[4k]u=[s/2]-K_{IB}\begin{bmatrix}b\\b\end{bmatrix}
=[s/2+4kb].
\]

Hence, for general boundary offset,

\[
u=b+\frac{s}{8k},\qquad
U=\begin{bmatrix}b&b+s/(8k)&b\end{bmatrix}^{T}.
\]

No boundary row is replaced and no boundary row remains in the reduced
residual contract.

## Frozen exact systems, solutions, residuals, and bounds

The exact absolute reduced-residual tolerance is

\[
\tau=2^{-30}=\frac1{1073741824}.
\]

The three systems are

| Point and exact tuple \((s,k,b)\) | Reduced system | Determinant | Interior solution | Full nodal solution | Exact reduced residual | \(\lVert A^{-1}\rVert_\infty\tau\) |
| --- | --- | --- | --- | --- | --- | --- |
| \(p_0=(2,1,0)\) | \([4]u=[1]\) | \(4\) | \(1/4\) | \([0,1/4,0]^T\) | \(4(1/4)-1=0\) | \(2^{-32}=1/4294967296\) |
| \(p_1=(4,1,0)\) | \([4]u=[2]\) | \(4\) | \(1/2\) | \([0,1/2,0]^T\) | \(4(1/2)-2=0\) | \(2^{-32}=1/4294967296\) |
| \(p_2=(2,5/4,0)\) | \([5]u=[1]\) | \(5\) | \(1/5\) | \([0,1/5,0]^T\) | \(5(1/5)-1=0\) | \(1/(5\,2^{30})=1/5368709120\) |

For a computed scalar \(\widehat u\) with true reduced residual
\(r=A\widehat u-f\),

\[
|\widehat u-u|=|A^{-1}r|
\leq\lVert A^{-1}\rVert_\infty |r|.
\]

The last column is therefore the residual-contract-induced solution-error
bound, not a floating-point ulp statement.

## Separate binary64 observations

Under nearest-binary64 round-to-nearest, ties-to-even:

| Point | Exact solution | Bits | Exact rational value stored | Stored minus exact | Exact true reduced residual of stored value |
| --- | --- | --- | --- | --- | --- |
| \(p_0\) | \(1/4\) | `0x3fd0000000000000` | \(1/4\) | \(0\) | \(0\) |
| \(p_1\) | \(1/2\) | `0x3fe0000000000000` | \(1/2\) | \(0\) | \(0\) |
| \(p_2\) | \(1/5\) | `0x3fc999999999999a` | \(3602879701896397/18014398509481984\) | \(1/90071992547409920=1/(5\,2^{54})\) | \(5\cdot1/(5\,2^{54})=2^{-54}=1/18014398509481984\) |

Around the stored \(p_2\) value the binary64 spacing is \(2^{-55}\), so its
half-ulp is \(2^{-56}\). Half-ulp bounds nearest representation error; it is
neither the absolute residual tolerance \(2^{-30}\) nor the
residual-conditioned solution-error bound \(1/(5\,2^{30})\).

## Required mutants

The structure-mismatch mutant is exactly

```text
offsets = [0, 2, 4]
columns = [0, 1, 0, 1]
values  = [4, -2, -2, 4]
rhs     = [1, 1]
```

so

\[
A_m=\begin{bmatrix}4&-2\\-2&4\end{bmatrix},\qquad
\det A_m=16-4=12,
\]

and

\[
A_m^{-1}\begin{bmatrix}1\\1\end{bmatrix}
=\frac1{12}\begin{bmatrix}4&2\\2&4\end{bmatrix}
\begin{bmatrix}1\\1\end{bmatrix}
=\begin{bmatrix}1/2\\1/2\end{bmatrix}.
\]

Its exact residual is `[0,0]`, but its 2-by-2 CSR pattern differs from the
accepted reduced 1-by-1 pattern `offsets=[0,1], columns=[0]`. Numerical
nonsingularity therefore does not cure the structural mismatch.

The same-pattern singular mutant is exactly

```text
offsets = [0, 1]
columns = [0]
values  = [0]
rhs     = [1]
```

Its determinant is zero. It preserves the accepted stored 1-by-1 pattern but
represents \(0u=1\), which is inconsistent and has no solution.

## Reuse and stale-factor proof

Every accepted reduced system has `offsets=[0,1]` and `columns=[0]`.

- From \(p_0\) to \(p_1\), the matrix value remains `[4]` while the right-hand
  side changes from `[1]` to `[2]`. The numeric factors are therefore reusable.
- From \(p_1\) to \(p_2\), the pattern remains the same but the matrix value
  changes from `[4]` to `[5]`. Only symbolic structure is reusable; the
  numeric factors must be refreshed.
- A stale \(p_0\) numeric inverse applies \(1/4\) to the \(p_2\) right-hand
  side `[1]`, producing \(u_{\mathrm{stale}}=1/4\). Against the true \(p_2\)
  system its exact residual is
  \(5(1/4)-1=1/4\), which exceeds \(2^{-30}\).

## Contract checks and nonclaims

- Tuple order is exactly `(source_scale,diffusion,boundary_offset)`; no tuple
  permutation is accepted.
- The source appears on the positive algebraic right-hand side because the PDE
  is \(-(ku')'-s=0\).
- The half-element stiffness scale is \(k/h=2k\), and two elements sum the
  interior diagonal to \(4k\).
- COO duplicates are coalesced and CSR columns are sorted within each row.
- Both endpoint contributions are shifted into the reduced right-hand side;
  boundary-row replacement is not used.
- Reuse classification depends on both exact CSR pattern and exact matrix
  values, not on solution coincidence or matrix nonsingularity.

This oracle makes no claim about a repository implementation, solver API,
runtime behavior, performance, pivoting, or factorization internals. It does
not cover nonuniform meshes, variable coefficients, other tuple orders,
higher-order or higher-dimensional elements, other quadrature rules, other
boundary conditions, or floating-point modes beyond the stated binary64
observation. No implementation, fixture, other oracle, or repository numerical
path was consulted in deriving these values.
