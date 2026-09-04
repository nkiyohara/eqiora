# Specimen: a resistor divider

This complete target-language specimen accompanies the [core specification](core.md).
The existing electrical packages implement scalar conserving networks, but the signatures,
named connector members, quantity literals, and simplified connection syntax below await
their language slices. This page is a specification, not a runnable current example.

## Use standard parts

With an exact dependency on `Eqiora.Electrical.Basic`, the intended source is:

```eqiora
import Eqiora.Electrical.Basic.basic as electrical;

model Divider() {
  instance source: electrical.IdealVoltageSource(voltage = 12 [V]);
  instance upper: electrical.Resistor(resistance = 1 [kOhm]);
  instance lower: electrical.Resistor(resistance = 2 [kOhm]);
  instance ground: electrical.Ground();

  connect source.positive, upper.positive;
  connect upper.negative, lower.positive;
  connect lower.negative, source.negative, ground.terminal;
}
```

The manifest and lock select the actual package; the import does not fetch it or select a
version. `kOhm` is a scaled input unit, while `Ohm` is the structural resistance dimension.
The divider is a continuous, lumped algebraic model. It has no spatial support, stored state,
initialization equation, clock, or hidden reference potential.

## Inspect the mathematics

Here is the complete same-file version, exposing the component definitions used above.
The connector and component names are ordinary declarations, not physics keywords.

```eqiora
connector Pin {
  across voltage: V;
  through current: A;
}

component IdealVoltageSource(
  parameter voltage: V,
  port positive: Pin,
  port negative: Pin
) {
  relation constitutive {
    positive.voltage - negative.voltage = voltage;
    positive.current + negative.current = 0 [A];
  }
}

component Resistor(
  parameter resistance: Ohm,
  port positive: Pin,
  port negative: Pin
) {
  relation constitutive {
    positive.voltage - negative.voltage = resistance * positive.current;
    positive.current + negative.current = 0 [A];
  }
}

component Ground(port terminal: Pin) {
  relation reference {
    terminal.voltage = 0 [V];
  }
}

model Divider() {
  instance source: IdealVoltageSource(voltage = 12 [V]);
  instance upper: Resistor(resistance = 1 [kOhm]);
  instance lower: Resistor(resistance = 2 [kOhm]);
  instance ground: Ground();

  connect source.positive, upper.positive;
  connect upper.negative, lower.positive;
  connect lower.negative, source.negative, ground.terminal;
}
```

For this scalar connector, `across` and `through` are typed member roles. Through current is
positive into each component occurrence. The connector owns those roles; the words `voltage`
and `current` only name the members. Each signature port is owned and exposed by its component
occurrence. It is not a missing named argument. The parameter is an external requirement.

Each net equates its across values and sums its signed through values to zero. The first two
nets each contribute one voltage equality and one current balance; the three-port ground net
contributes two voltage equalities and one current balance. The components contribute their
own five equations. Seven ports carry fourteen scalar values, and the complete model has
fourteen scalar equations. This count is a useful closure check, not a general solvability test.

Ground prescribes voltage only. Adding a zero-current equation inside Ground would duplicate
a consequence of this closed circuit and could overconstrain a different composition. An
unconnected port does not silently acquire a zero-current condition.

## Independent predictions

Let the grounded potential be zero, the source potential be 12 V, and the divider midpoint be
`v`. The resistor equations and the midpoint current balance give:

```text
I = (12 V - v) / (1000 Ohm) = v / (2000 Ohm)
I = 12 V / (3000 Ohm) = 0.004 A
v = I * (2000 Ohm) = 8 V
```

The upper drop is 4 V. Both resistor positive-port currents are +4 mA and both negative-port
currents are -4 mA. The source positive-port current is -4 mA; its negative-port current is
+4 mA. Ground terminal current is zero as a result of the network, not a default law.

The resistors absorb 16 mW and 32 mW. The ideal source absorbs -48 mW, so total absorbed power
is zero. These values follow from Ohm's law and the declared connection orientation, independently
of compiler output or solver choice.

The initial numerical test should check the voltage differences, signed port currents, and
power balance with dimension-appropriate tolerances. It should not infer success from equation
count alone or bless a generated residual snapshot as the expected mathematics.

## Rejections and mutations

| Change | Required outcome |
|---|---|
| Bind `resistance = 1 [V]` | Static dimension error at the binding |
| Bind `resistance` twice | Duplicate binding error identifying both occurrences |
| Omit `voltage` from the source occurrence | Missing required parameter |
| Read `upper.current` | Unknown member; the current belongs to a named port |
| Connect a foreign nominal connector with the same units | Connector compatibility error |
| Remove Ground and its terminal from the net | No hidden reference; numerical admission must detect the floating system |
| Reverse one port's declared current contribution in the equations | The independently derived signed currents or power balance expose the changed model |

Reordering constitutive equations must not create assignment sequencing. Renaming a connector
member changes source lookup, but retaining its typed role must preserve the connection law.
The emitted and reparsed source must retain port occurrences, connector identity, and equation
ownership. Package-qualified and same-file definitions are separate source identities; comparison
of their mathematics uses the shared mathematical projection, not byte equality.
