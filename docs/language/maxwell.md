# Specimen: a lossless Maxwell cavity

This target-language model describes a bounded, real, three-dimensional Maxwell cavity.
Vector calculus and compatible numerical execution remain separate implementation slices;
the source below is specified, not currently executable as a complete workflow.

```eqiora
model MaxwellCavity(
  support body: volume(ambient_dimension = 3),
  support walls: complete_exterior(parent = body),
  parameter permittivity: F / m,
  parameter permeability: H / m,
  input initial_electric: vector<V / m, 3> on body
) {
  state electric: vector<V / m, 3> on body;
  state magnetic: vector<A / m, 3> on body;

  initial {
    electric = initial_electric;
    magnetic = 0;
  }
  relation evolution on body {
    permittivity * derivative(electric) = curl(magnetic);
    permeability * derivative(magnetic) = -curl(electric);
  }
  relation gauss on body {
    div(permittivity * electric) = 0;
    div(permeability * magnetic) = 0;
  }
  relation conductor on walls {
    tangential_trace(electric) = 0;
    normal_trace(permeability * magnetic) = 0;
  }
  observable energy: J = integral(
    0.5 * (permittivity * inner(electric, electric) + permeability * inner(magnetic, magnetic)),
    measure(body)
  );
}
```

Bind positive real constant permittivity and permeability, a fixed cube `[0,L]^3` with `L>0`,
its exact six-face exterior, and one right-handed Cartesian frame. The medium is isotropic,
linear, nondispersive, and lossless; volume charge and current are zero. No surface condition
silently sets the electric normal component to zero: conductor surface charge can support it.

`initial_electric` supplies the exact spatial field used at fresh initialization. It is not
a continually imposed electric source, a solver guess, or another evolved state. Only its
initial value is consumed; restart consumes the accepted electric/magnetic fields instead.
For the complete binding below it is time independent and compatible with Gauss and PEC.
The zero magnetic initializer is contextual vector zero in the declared frame and support.

The Gauss equations and boundary constraints remain explicit mathematical requirements.
Formulation must recognize their constraint-preservation role through its admitted Maxwell
pair; blindly appending them as unrelated scalar evolution equations does not establish a
square or executable system. No automatic divergence cleaning or energy rescaling is implied.

## Spatial operators and orientation

For components in the declared right-handed frame,
`curl(a) = (partial_y(a_z)-partial_z(a_y), partial_z(a_x)-partial_x(a_z),
partial_x(a_y)-partial_y(a_x))`. Curl divides dimensions by length and preserves the spatial
vector role on the same support. `div(a)` contracts the spatial derivative and vector axes.
`cross(a,b)` uses that same orientation, multiplies dimensions, and introduces no conjugation.
Neither operation silently embeds a two-dimensional array into three dimensions.

On an exact parent-outward boundary, `normal_trace(a)` is `n dot a` and
`tangential_trace(a)` is `n cross a`. These canonical names deliberately distinguish the
oriented tangential trace from the tangential projection `a - n*(n dot a)`.
Outside a boundary relation, supply `on = boundary`; at a two-sided interface also supply
`from = parent_support`. A general weak trace requires its admitted function-space meaning,
not necessarily a full pointwise vector boundary value.

The integration-by-parts identity is
`integral((curl a) dot b - a dot (curl b), body) = integral((n cross a) dot b, boundary)`
for smooth real fields. Complex weak forms retain their explicit conjugation. Reversing
the common orientation changes the boundary sign. The classical identities `div(curl(a))=0`
and `curl(grad(f))=0` need commuting derivatives under the admitted regularity; source typing
does not prove that an arbitrary discretization preserves them.

## Complete analytic initial field and evolution

Let `k = pi/L`, `omega = sqrt(2)*k/sqrt(permittivity*permeability)`, and choose real amplitude
`E0` with units V/m. Bind the initial field to `(0, E0*sin(k*x)*sin(k*z), 0)` in the exact
cube frame. The analytic solution measured from fresh time zero is:

```text
E_y = E0*sin(k*x)*sin(k*z)*cos(omega*t)
H_x =  E0*k/(permeability*omega)*sin(k*x)*cos(k*z)*sin(omega*t)
H_z = -E0*k/(permeability*omega)*cos(k*x)*sin(k*z)*sin(omega*t)
E_x = E_z = H_y = 0
```

Taking the written curls directly gives both evolution equations. The electric divergence
vanishes because E_y is independent of y. The x and z derivatives in magnetic divergence
cancel. At x and z walls the electric tangential value vanishes; at y walls the electric
field is normal. Magnetic normal values vanish on every wall. Thus the binding satisfies
the entire exterior, rather than only two selected faces.

At initialization the magnetic field is zero and the integrated energy is
`permittivity*E0^2*L^3/8`. The electric and magnetic energies subsequently equal this constant
times `cos(omega*t)^2` and `sin(omega*t)^2`, respectively. At a quarter period the electric
field vanishes and magnetic energy is maximal. At half a period the electric field is the
negative initial field and the magnetic field vanishes. Energy conservation alone cannot
distinguish the half-period phase reversal from an unchanged initial state.

The Poynting flux is `cross(electric, magnetic)` with dimension W/m^2. Its outward wall
flux is zero for this PEC mode. The volume energy balance follows from the paired curl
equations and the oriented boundary identity, not from a display renderer or a field-name rule.

## Admission and independent failures

An implementation must separately admit its spatial curl/div pair, time integration method,
and step-size policy. Refining output timestamps does not refine either numerical method.
Restart preserves the same support/frame and accepted state/timeline; an unrelated mesh or
stale boundary handle cannot be substituted because the field arrays have the same size.

Reversed curl signs change the trajectory; wrong handedness changes oriented components;
swapped x/z factors fail the signed field reference; omitted Gauss constraints permit invalid
initial data; adding a zero electric-normal condition rejects this legitimate mode. Check
phase and signed field components alongside energy, Gauss residuals, and boundary flux.
A selected representation that lacks the required trace or compatible vector calculus must
reject before Run, without claiming that syntax alone delivers Maxwell accuracy.
