# Issue #407 Stokes dissipation v2 route reconciliation v1

## Verdict

**AGREEMENT / ACCEPT.** The sealed analytic/discrete v2 route and sealed
independent numerical v2 route agree on every comparison required by the
successor evidence contract and the effective v3 route-output schema. No
required item is missing or unresolved, and no value was averaged, selected
between routes, or admitted by a widened tolerance.

This accepts only the two-route scientific reconciliation. It is not Eqiora
candidate acceptance, implementation authority, fixture acceptance,
publication authority, or an optimum/performance claim.

## Exact inputs consumed

All route hashes were checked before either route was opened. The supplied
successor-contract hash was also checked before that authority was opened.

| Input | SHA-256 | Check |
| --- | --- | --- |
| `/data/nk523/.tmp/issue407-stokes-dissipation-analytic-discrete-route-v2.md` | `a8a5b8d46cd47beb8e61afb1ee13bf47812688437fff09be8702f608d3b98758` | Supplied digest matched; regular mode `0444` |
| `/data/nk523/.tmp/issue407-stokes-dissipation-independent-numerical-route-v2.md` | `b6c706e1d6a100b9c3fffb3da0b430ee7ee3374ac99d0959598156cec8d8acbf` | Supplied digest matched; regular mode `0444` |
| `/data/nk523/.tmp/issue407-stokes-dissipation-evidence-successor-v1.md` | `0129d3cd9d011ac320bf9461b259e00923a1403fe03fee356ba658dee059f0b0` | Supplied digest matched |
| `/data/nk523/.tmp/issue407-stokes-dissipation-route-input-schema-amendment-v3.md` | `b3405130dc73d59cac333941a33d5d79671e2f06913791146d085ccebbb291f9` | Computed digest; equals the digest accepted by its review |
| `/data/nk523/.tmp/issue407-stokes-dissipation-route-input-schema-amendment-v3-review.md` | `feb6044e73bef1edf313ebb56ab885913d7bd38366d0957e337c8b27da192c2b` | Computed digest; ACCEPT review |
| `/data/nk523/.tmp/issue407-stokes-dissipation-sealed-inputs-v2-review.md` | `df3de8184526c01f0be57452d8f416087d837a4d748b30aed041ee4b8b460eec` | Computed digest; ACCEPT review for the exact two v2 route paths |

Repository policy was read from `/data/nk523/projects/eqiora/AGENTS.md`,
SHA-256 `482764959fe4209e4a221e731ce7f11069591628e19ff2cda171a8f7acc5d405`.
Both routes declare the same transitive sealed scientific input identity,
`478235237851d70f2c5d411b57d0d068ea2786b44005d17b6ad089a21a28c463`;
the accepted sealed-input review binds that identity. The JSON itself was not
opened by this reconciler.

## Governing comparison matrix

`A` below means the analytic/discrete route and `N` means the independent
numerical route. Decimal differences below are comparisons of reported
binary64 observations, not recomputed Stokes results.

| Required comparison | A statement/observation | N statement/observation | Predicate | Outcome |
| --- | --- | --- | --- | --- |
| Authority and input identity | Exact v2 sealed-input SHA, v3 amendment/review, and exact-input review | Same identities | Exact strings/hashes | **AGREEMENT** |
| Route isolation | No v1 route/reconciliation, other v2 route, Eqiora implementation/candidate/fixture, or writer scratch read | Same isolation statement | Contract isolation | **AGREEMENT** |
| Independent method identity | Finite-dimensional tangent plus adjoint JVP/VJP, checked by fresh centered differences | Independently assembled full augmented MINI/P1 systems and complete centered differences with two-finest extrapolation | Methods must answer the same sealed question without sharing Eqiora output; method strings need not match | **AGREEMENT** |
| Source versus specialization classification | Treats the fixed topology, harmonic map, MINI/P1 residual, 12-point quadrature, derivative, and history as the sealed finite-dimensional benchmark; continuous shape calculus is expressly superseded | Treats the same objects as sealed benchmark inputs; makes no external-source value claim | No Eqiora specialization or observed value attributed to Pironneau/Richardson; no drag/force/exterior claim | **AGREEMENT**. `Richardson extrapolation` in N names a numerical extrapolation operation, not the excluded Richardson-1995 physical claim |
| Geometry/profile/area | Normalized polar body, exact-coordinate regeneration, analytic-area mutant rejected before meshing | Same regeneration and same rejection stage | Exact profile identity; analytic-area predicate | **AGREEMENT** |
| Normal and derivative sign | Body/fluid sign mutant rejected at analytic/discrete sign comparison | Same identity, verdict, and stage | Exact sign convention and complete derivative sign | **AGREEMENT** |
| Outer boundary | Four-side all-Dirichlet interpretation; traction/inlet-outlet mutant rejected before solve | Same identity, verdict, and stage | Exact boundary contract | **AGREEMENT** |
| Pressure gauge | One zero-mean gauge in the augmented system; omit/duplicate mutant rejected before objective use | Full augmented system; same mutant verdict/stage | Gauge/residual admission | **AGREEMENT** |
| Objective formula and unit | `E_h = u^T A u` with the `2 mu epsilon:epsilon` factor; `W/m` | Same sealed objective; factor-loss mutant rejected at independent value comparison | Objective formula/unit and sealed mixed-value predicate | **AGREEMENT** |
| Complete discrete gradient | Differentiates boundary regeneration, harmonic motion, maps, weights, state, bubble terms, and objective; tangent/adjoint agree | Every centered sample regenerates coordinates, resolves harmonic motion, reassembles, and resolves state | Both coordinates and every sealed direction under mixed tolerance/trend predicates | **AGREEMENT** |
| Ordinary positive path before mutants | Start solve, all derivative probes, rejected trial 0, accepted nonzero trial 1, terminal, and refined ordering completed first | Same ordered positive path completed before all mutants | Exact ordinary-positive ordering | **AGREEMENT** |
| Sufficient decrease | Trial 1 at `['3/8','0']` accepted; margin `0.81994152365871997 W/m` | Same exact trial/design/outcome; margin `0.81994152365867023 W/m`, guard `1.4194422058758371e-7 W/m` | Exact Armijo structure/decision and positive guarded margin | **AGREEMENT**; margin difference `4.9738e-14 W/m` |
| Complete history and associations | Two trials; order 0 rejected outside design set, order 1 accepted; exact native association projection | Byte-identical projection | Exact v3 structural grammar and digest | **AGREEMENT** |
| Terminal | Gradient infinity norm `11.673011086134991 W/m`; `budget exhaustion` | `11.673011086060706 W/m`; same disposition | Stationarity threshold `1e-6 W/m`; exact disposition/predicate | **AGREEMENT**; both are non-stationary and terminate by budget exhaustion |
| Distinct refined topology and association | `stokes-square-ring-refined-n64-m8-v1`, start then accepted-final | Same identity/design ordering | Exact distinct identity/association before strict objective ordering | **AGREEMENT** |
| Resource bounds | `73,405,555 / 2,000,000,000` abstract units; every event cap PASS | `68,991,933 / 2,000,000,000`; every listed cap PASS | Each route must independently stay within its sealed caps; counts need not be equal | **AGREEMENT** |
| Nonclaims | No continuous-shape replacement, route-native history equality, candidate, fixture, integration, performance, or optimum claim | No continuum convergence, drag, force, optimizer convergence, stationarity, general-shape, or production claim | Contract nonclaims preserved | **AGREEMENT** |

## Coordinate record comparison

The two machine-authoritative blocks contain the same 40 records in the same
order. For every record, `design`, `observation_identity`,
`topology_content_identity`, `topology_role`, `unit`, and `vertex_count` are
exactly equal. All `14,464` reported coordinate components are byte-for-byte
equal (`max_abs_delta = 0 m`), which is stronger than both inherited
componentwise coordinate predicates (`2e-12 m` and `2e-11 m`). The canonical
preimage SHA-256 for each record was replayed independently from each route;
all 80 replays passed, and corresponding route digests are equal.

| Observation identity | Agreed/replayed preimage SHA-256 | Outcome |
| --- | --- | --- |
| `reference/start` | `9a2ad1df81f299365a24aa95b3544a16709ba37c717fee0543ccb1167d72c0bc` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/coordinate-a2/2e-3/minus` | `c8c00863924754c44936e883ff414f866fb1da5838bea750d4fdfc42983db4e5` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/coordinate-a2/2e-3/plus` | `22a95abe529bae287245722002356b329ef4d921194ff138889519792852e73e` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/coordinate-a2/1e-3/minus` | `18a4c39d4b8fd4c5ac9c65793712b1282ed0b63915df4aa0ffd1041aa64ca9a5` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/coordinate-a2/1e-3/plus` | `a4d90d734a35d3015cc5f2eb1e672d14db9ded8b680bc92adf05b8a7bb892267` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/coordinate-a2/5e-4/minus` | `62e4570293b8d30cbf5a7cca77cec5b0427540a9146596def37073902e3bfe6e` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/coordinate-a2/5e-4/plus` | `3b8136416a0fee01f750643148e5552cfffa47ce4e0042a280cc64ecc91cfae4` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/coordinate-a4/2e-3/minus` | `cb99b75e72c9d95e376592d9069c6499c0f4b244fd191e059ad1927b6edd1ed4` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/coordinate-a4/2e-3/plus` | `81aa63a4f934477eb492e0f0250126839f8dcb5f3003717bba56523272d68352` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/coordinate-a4/1e-3/minus` | `6f8950a1edaa422fb3be85023b0c72d078a4a84dfb19c53bb084c2cc2ec08d04` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/coordinate-a4/1e-3/plus` | `afb97dae321b828218b4facc156f4db6a46cae2bd751412072fcbe84f362534f` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/coordinate-a4/5e-4/minus` | `0909886955bf9559c1dbf347d3e7f970cbb802419b43afd78fa20ab3bac17f0f` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/coordinate-a4/5e-4/plus` | `f87181246f9600e5523a8846522530a5a87692343275e0e6c120fe64757d1faf` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/direction-3-4-5/2e-3/minus` | `ea66b0f43b042788431a78ba581c22c6c10e32278e2add3770b49a63a5f4c2f9` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/direction-3-4-5/2e-3/plus` | `615d7a60c28356e0fd5e8c7672624bbee1208a12842dc165aefaa757481327d8` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/direction-3-4-5/1e-3/minus` | `408db491f525ff3407139682aa5ed01d2c0953128cf248ce8e0cd7a4c67cc081` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/direction-3-4-5/1e-3/plus` | `64b193d72efd4625959dd4380f8ce1cefa85a6a909589fd37bdc5b7aafebe87e` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/direction-3-4-5/5e-4/minus` | `be7041bef31b92907345ef3d12a8be8b982babf1fc2b4c92b8f3fef1e32db203` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/direction-3-4-5/5e-4/plus` | `5d1d8a8a7ca94cfb2c063e41bfce72f7cbce21cdfc25ed8e2dd522bb535bba61` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/direction-minus4-3-5/2e-3/minus` | `e2e7f4801a2d401f07ca461e1352f4a9ec14705cd2b47f6c76da73bbcabbfe7c` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/direction-minus4-3-5/2e-3/plus` | `05a4bb49b50906b52fd549502ebdcf6296e83eb5d9b8a43a267f7d6f865610d4` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/direction-minus4-3-5/1e-3/minus` | `a385d28266c2d870a15dc0094e5ff91cb1ed681736a44a53f18298e0035777fb` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/direction-minus4-3-5/1e-3/plus` | `10a47eb0748aac87857a4b9fe0dc222d48c27715b20307e2f6413a03de161d2a` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/direction-minus4-3-5/5e-4/minus` | `ecdbf7ca1e0d3cfee26b32386f012e58fae9d479dc9fc688656d4c8da5b63bf9` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/start/fd/direction-minus4-3-5/5e-4/plus` | `2af61d0a2b64868a81db2f3877489c52dbf121ffeb919ab837ea1db58a13da68` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/history/trial/1` | `4fb6ea75c4dc8105da559946c2d33fbef577cc51813fb0f734bde7ff8ae9d61d` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/accepted-final/fd/coordinate-a2/2e-3/minus` | `7f67607eac7d9ed9d5a963f634df51feba9232dca77787887ec70d57cea20ae1` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/accepted-final/fd/coordinate-a2/2e-3/plus` | `6fd4ae66766b73924da406b5a0e29f38c3fda083edbc988161345702e9d726d6` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/accepted-final/fd/coordinate-a2/1e-3/minus` | `aeb2b6dbbcfd0eb7b2234b096d2b69358215e144e5b566128dd6f06639dcd846` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/accepted-final/fd/coordinate-a2/1e-3/plus` | `92fbb5d86ef3f919f364a0d4f498e603e7ac812aa6302b68f14933f96c7eaab4` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/accepted-final/fd/coordinate-a2/5e-4/minus` | `9706cf49c74302c14a59ffabe60ac09f09b13acbcbdfff6a238b8df25535f698` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/accepted-final/fd/coordinate-a2/5e-4/plus` | `5b02a97ec4a3b6a451a1dd6b32c95abb0b8cfc552fdfbb7f315233cbba504264` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/accepted-final/fd/coordinate-a4/2e-3/minus` | `8ee1953a6a3578decad6d4fd51a8532126183d89bc723c8f54ccca88bc3c13ba` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/accepted-final/fd/coordinate-a4/2e-3/plus` | `39efc20c63df4dffbbc14eb0439857a7bbaa8470cc3888cd0b52b890ab8fb995` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/accepted-final/fd/coordinate-a4/1e-3/minus` | `e3e6f56f6d019ff275cb69c7ce28ae996d2f485a9ed84e3f9f82fb325845efe0` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/accepted-final/fd/coordinate-a4/1e-3/plus` | `ffad00617cdf1ac36a2f21422a24529ee21def5faff5a9f2e5cc016c056f6985` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/accepted-final/fd/coordinate-a4/5e-4/minus` | `4b6fac9d4972dd59b40b1aba71c5b4ae24f61f1f44d8b7bee34528480e2dd932` | Exact metadata and component-byte equality; both digest replays PASS |
| `reference/accepted-final/fd/coordinate-a4/5e-4/plus` | `cd49a2f4f82035b855b4769ae4aa4cf80a3493e08dd7b6d931df93a110e3ae0a` | Exact metadata and component-byte equality; both digest replays PASS |
| `refined/start` | `f5b064f5f0356e9c66b2912217c69716922974697a92caea582e573636a717fa` | Exact metadata and component-byte equality; both digest replays PASS |
| `refined/accepted-final` | `bb3b9b768ca2345a2bff23754005157523bb5f04c742a9bb147ba1c58f31a5a3` | Exact metadata and component-byte equality; both digest replays PASS |

## Canonical history comparison

| Required structural item | A | N | Outcome |
| --- | --- | --- | --- |
| Projection schema/root | `issue407-stokes-history-identity-v2`; the exact four v3 root keys | Byte-identical | **AGREEMENT** |
| Accepted orders | `[1]` | `[1]` | **AGREEMENT** |
| Trial 0 | Order/key `0`/`trial/0`; parent `start`; direction `['1','0']`; step `3/4`; coefficient admission `fail`, all later flags `not_evaluated`; disposition `outside design set`; all child associations/real references null | Byte-identical | **AGREEMENT** |
| Trial 1 | Order/key `1`/`trial/1`; parent `start`; direction `['1','0']`; step/design `3/8`/`['3/8','0']`; all nine flags `pass`; disposition `accepted` | Byte-identical | **AGREEMENT** |
| Armijo structure | Exact `c1='1/10000'`, start objective/directional references, null child for trial 0, trial-1 dissipation reference and `pass` for trial 1 | Byte-identical | **AGREEMENT** |
| Association structure | Exact model/topology, parent derivative, and null/current-trial correspondence/geometry/mesh/realization/run/result bindings under v3 presence rules | Byte-identical | **AGREEMENT** |
| Real-observation references | Exact five-key maps, exact nulls for trial 0, exact five trial-1 references | Byte-identical | **AGREEMENT** |
| Terminal | After order `1`; `budget exhaustion`; final derivative bound to `reference/history/trial/1`, design `['3/8','0']`, exact reference topology; stationarity `fail` | Byte-identical | **AGREEMENT** |
| Complete projection bytes | Canonical projection JSON equals N | Canonical projection JSON equals A | **AGREEMENT** |
| Positive digest | Reported `dfc1c9f568a1c71c75c32023cd8dc7b773e07e5130949fa9cd8056ea62baf669`; independent replay matches | Same; independent replay matches | **AGREEMENT** |
| Required swap-mutant digest | Reported `d48d383c05855234bdde0151ac3e4dc8cb51e023d4cd8ad2a7e2ced4e6a746c9`; independent replay matches and differs from positive | Same; independent replay matches and differs from positive | **AGREEMENT** |
| Swapped-order rejection | Swapped entries retain embedded order/key and fail `array index == trial_order` | Same | **AGREEMENT** |

The outer route-report schemas and route-native method/state identities are
deliberately different and are not part of the exact history preimage. No
translator was needed for the mandated projection.

## Real observations, objectives, residuals, gauges, areas, and units

All eleven `history_real_observations` keys, their order, and their units are
exactly equal. Banded fields are compared under the unchanged sealed
real-field predicate referenced by both reports; exact fields are compared by
bits. The start-objective tolerance reproduced by the numerical route is
`2.8488844117516743e-7 W/m`; the state-residual upper bound reproduced by the
stale-state falsifier is `5e-10`. No new threshold was selected here.

| Observation | Unit | A | N | Absolute difference / predicate | Outcome |
| --- | --- | ---: | ---: | --- | --- |
| `start/dissipation` | `W/m` | 14.194422058758359 | 14.194422058758370 | `1.0658141036401503e-14`; sealed mixed objective tolerance | **AGREEMENT** |
| `start/gradient/a2` | `W/m` | -5.7410127747219413 | -5.7410127747242372 | `2.2959412149248237e-12`; sealed mixed gradient tolerance | **AGREEMENT** |
| `start/gradient/a4` | `W/m` | -3.6696943004879352 | -3.6696943004178215 | `7.0113692629547586e-11`; sealed mixed gradient tolerance | **AGREEMENT** |
| `start/gradient/dot-search-direction` | `W/m` | -5.7410127747219413 | -5.7410127747242372 | `2.2959412149248237e-12`; direction is exactly `['1','0']` | **AGREEMENT** |
| `trial/1/analytic_area` | `m^2` | 3.1415926535897931 | 3.1415926535897931 | Exact binary64 bits | **AGREEMENT** |
| `trial/1/polygonal_area` | `m^2` | 3.1058360078925609 | 3.1058360078925609 | Exact binary64 bits; derived polygon predicate | **AGREEMENT** |
| `trial/1/state_residual` | `1` | 4.5087917749511199e-15 | 3.2776038872277526e-15 | Both pass unary residual admission; difference `1.2311878877233673e-15` | **AGREEMENT** |
| `trial/1/pressure_gauge` | `1` | 9.3988516364647481e-17 | 7.0491387273485392e-17 | Both pass unary gauge admission; difference `2.3497129091162089e-17` | **AGREEMENT** |
| `trial/1/dissipation` | `W/m` | 13.374265247120587 | 13.374265247120647 | `6.0396132539608516e-14`; sealed mixed objective tolerance | **AGREEMENT** |
| `terminal/gradient/a2` | `W/m` | 1.5584049861836824 | 1.5584049861034899 | `8.0192519291699682e-11`; sealed mixed gradient tolerance | **AGREEMENT** |
| `terminal/gradient/a4` | `W/m` | -11.673011086134991 | -11.673011086060706 | `7.4285466666879074e-11`; sealed mixed gradient tolerance | **AGREEMENT** |

The additional required primal/refined observations also agree:

| Observation | A | N | Required relation | Outcome |
| --- | ---: | ---: | --- | --- |
| Reference-start state residual | 5.1955515310647503e-15 | 2.5174607198162076e-15 | Finite and within sealed residual admission | **AGREEMENT** |
| Reference-start pressure gauge | 1.118954915617531e-16 | 1.6224846276454103e-17 | One-gauge convention and unary admission | **AGREEMENT** |
| Refined-start dissipation (`W/m`) | 10.884735042378647 | 10.884735042378749 | Mixed value predicate; difference about `1.02e-13` | **AGREEMENT** |
| Refined-final dissipation (`W/m`) | 10.339394919857694 | 10.339394919857593 | Mixed value predicate; difference about `1.01e-13` | **AGREEMENT** |
| Refined-start state residual | 3.8371747751130437e-15 | 5.013973380960068e-15 | Finite and within sealed residual admission | **AGREEMENT** |
| Refined-final state residual | 7.0320298345158903e-15 | 4.708931160204648e-15 | Finite and within sealed residual admission | **AGREEMENT** |
| Refined decrease (`W/m`) | 0.54534012252095287 | 0.54534012252115538 | Strictly positive and above route-specific required margin | **AGREEMENT** |
| Required refined margin (`W/m`) | 1.0884735042378647e-5 | 1.0884735042378747e-5 | Each reported value applied to its own start observation | **AGREEMENT** |

## Gradient and finite-difference probe comparison

Directions, selectors, step sequence `2e-3, 1e-3, 5e-4`, plus/minus design
identities, and all coordinate-record SHA references are exact. Each row below
covers all three steps and both independently regenerated signs. The reported
mixed tolerances for the four start probes are shown exactly. Both routes
state that all start and accepted-final mixed-tolerance and trend predicates
pass.

| Selector/probe and direction | A complete derivative (`W/m`) | N extrapolated derivative (`W/m`) | Route difference | Sample-value comparison across all three steps | Tolerance/trend outcome |
| --- | ---: | ---: | ---: | --- | --- |
| Start `coordinate-a2`, `[1,0]` | -5.7410127747219413 | -5.7410127747242372 | 2.2959412149248237e-12 | Six objective values: max difference `6.22e-14 W/m`; three centered derivatives: max difference `4.09e-11 W/m` | Mixed tolerance `1.1487025549448476e-3 W/m`; PASS |
| Start `coordinate-a4`, `[0,1]` | -3.6696943004879352 | -3.6696943004178215 | 7.0113692629547586e-11 | Six objective values: max difference `3.91e-14 W/m`; three centered derivatives: max difference `3.46e-11 W/m` | Mixed tolerance `7.344388600835643e-4 W/m`; PASS |
| Start `direction-3-4-5`, `[3/5,4/5]` | -6.3803631052235126 | -6.3803631053061709 | 8.2658324629392155e-11 | Six objective values: max difference `7.64e-14 W/m`; three centered derivatives: max difference `7.46e-11 W/m` | Mixed tolerance `1.27657262103376e-3 W/m`; PASS |
| Start `direction-minus4-3-5`, `[-4/5,3/5]` | 2.3909936394847926 | 2.3909936394046483 | 8.014433561243095e-11 | Six objective values: max difference `7.64e-14 W/m`; three centered derivatives: max difference `7.99e-11 W/m` | Mixed tolerance `4.7869872790573954e-4 W/m`; PASS |
| Accepted-final `coordinate-a2`, `[1,0]` | 1.5584049861836824 | 1.5584049861034899 | 8.0192519291699682e-11 | Six objective values: max difference `6.39e-14 W/m`; three centered derivatives: max difference `6.93e-11 W/m` | Sealed mixed tolerance and trend predicate; both reports PASS |
| Accepted-final `coordinate-a4`, `[0,1]` | -11.673011086134991 | -11.673011086060706 | 7.4285466666879074e-11 | Six objective values: max difference `8.53e-14 W/m`; three centered derivatives: max difference `1.42e-11 W/m` | Sealed mixed tolerance and trend predicate; both reports PASS |

Thus every one of the 36 plus/minus objective observations and every one of
the 18 centered-difference observations was compared. The largest derivative
or directional route difference is `8.2658324629392155e-11 W/m`; no sealed
band was widened.

## Falsifier comparison

Both routes first completed the ordinary positive path. All thirteen mutants
are therefore non-vacuous. Identity, rejected verdict, and required rejection
stage agree exactly for every mutant.

| # | Mutant | Required and reported rejection stage | Outcome |
| ---: | --- | --- | --- |
| 1 | `area-normalization-denominator-sign-or-omission` | analytic area/profile check before meshing | Both **REJECTED**; exact stage agreement |
| 2 | `swap-a2-a4-or-wrong-harmonic-angle` | regenerated geometry and coordinate/directional derivative comparison | Both **REJECTED**; exact stage agreement |
| 3 | `body-expansion-uses-fluid-normal-sign` | analytic/discrete sign comparison | Both **REJECTED**; exact stage agreement |
| 4 | `outer-side-inlet-outlet-or-traction-substitution` | boundary-contract admission before solve | Both **REJECTED**; exact stage agreement |
| 5 | `pressure-gauge-omitted-or-duplicated` | gauge/residual admission before objective use | Both **REJECTED**; exact stage agreement |
| 6 | `dissipation-factor-from-2mu-epsilon-epsilon-omitted` | objective formula/unit and independent value comparison | Both **REJECTED**; exact stage agreement |
| 7 | `geometry-map-quadrature-or-state-held-fixed-in-derivative` | complete reduced-gradient comparison | Both **REJECTED**; exact stage agreement |
| 8 | `perturb-deformed-polygon-not-analytic-rho` | exact geometry identity before finite-difference comparison | Both **REJECTED**; exact stage agreement |
| 9 | `stale-parent-state-run-or-result` | exact child Geometry/Mesh/Run/Result association and independently replayed child-state residual before objective or acceptance | Both **REJECTED**; exact mandatory stage agreement |
| 10 | `reference-aliased-as-refined-or-designs-swapped` | distinct topology identities and correct design/objective association before refined ordering | Both **REJECTED**; exact mandatory stage agreement |
| 11 | `decreasing-objective-bypasses-invalid-mesh` | geometry/mesh admission before solve or sufficient decrease | Both **REJECTED**; exact stage agreement |
| 12 | `rejected-trial-deleted-overwritten-or-reordered` | immutable complete-history order and digest | Both **REJECTED**; exact stage agreement and digest replay |
| 13 | `budget-exhaustion-relabelled-stationarity` | stationarity predicate and terminal disposition | Both **REJECTED**; exact stage agreement |

## Agreed oracle observations frozen by this reconciliation

- Exact coordinate records and their 40 SHA-256 values are the agreed records
  listed above.
- The exact canonical history projection and positive/mutant digests are the
  agreed structural history.
- Exact shared history fields are the keys, units, analytic and polygonal area
  bits, trial/order/association structure, accepted design `['3/8','0']`, and
  terminal disposition `budget exhaustion`.
- Banded scientific observations are the two reported route values shown
  above, accepted only under the precommitted sealed predicates. This document
  does not average them or nominate one route's bit pattern as preferred.
- The refined claim is only strict accepted-final-below-start ordering on the
  exact distinct refined topology.

## Nonchecks

- No Stokes, harmonic-motion, mesh, quadrature, derivative, optimizer, or
  mutant science was recomputed or rerun. Binary64 decoding, absolute
  route-to-route differences, canonical JSON digest replay, and exact byte
  comparisons were reconciliation operations only.
- The sealed v2 JSON, predecessor v1/v2 amendments, v1 routes/reconciliation,
  source PDF, route programs, and any route or writer scratch were not opened.
- No Eqiora implementation, candidate output, fixture, expected oracle,
  source map, public API, branch, pull request, Issue state, or GitHub state was
  inspected or changed.
- No repository test, `mise` gate, hosted CI check, packaging check, or
  publication/rendering check was run.
- Route-native state-vector hashes and route-native report framing were not
  compared for equality; the governing contract makes them diagnostic rather
  than cross-route observations.
- No continuous-shape derivative, remesh derivative, force/drag equivalence,
  Richardson-1995 exterior-flow claim, mesh-independent result, stationarity,
  local/global optimum, convergence, performance, portability, or resource-
  residency claim was checked or inferred.
- This reconciliation did not test an Eqiora candidate and does not establish
  implementation, integration, fixture, gallery, or publication acceptance.
