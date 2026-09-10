# org.example.closed_circuit

A third-party-shaped application Model Package that depends only on the exact
public interface of `Eqiora.Electrical.Circuits`. It instantiates the composed
`ParallelDc` component and binds its three scalar parameters.

The package exists to verify transitive, exact-offline component reuse. It has
no privileged resolver path and does not reach through the intermediate
package to `Eqiora.Electrical.Basic`.
