# RFC 0074: Eqiora public alpha identity

- Status: accepted
- Scope: public project and package identity
- Decision date: 2026-07-23

## Decision

The first public alpha keeps the name **Eqiora** and publishes one coherent
identity:

| Surface | Identity |
|---|---|
| Project | Eqiora |
| Website | `https://eqiora.org` |
| Source repository | `https://github.com/nkiyohara/eqiora` |
| Python distribution | `eqiora` |
| Python import namespace | `eqiora` |
| Rust package prerelease | `0.1.0-alpha.1` |
| Python normalized prerelease | `0.1.0a1` |
| Git tag | `v0.1.0a1` |
| License | Apache-2.0 |

The package version is authored once in the Cargo workspace. Maturin performs
the SemVer-to-PEP-440 normalization. Schema, wire, artifact, and evidence
domain versions remain independent compatibility identities.

## Why Eqiora

Eqiora is short, pronounceable in the project's working languages, and
retains continuity with the design records, package names, and evidence
catalogue developed before the public alpha. It also suggests equations and
equilibrium without narrowing the project to one numerical method or physics
domain.

Names such as Relata remain semantically attractive, but a rename immediately
before the first alpha would add migration risk without evidence of an actual
conflict. A future rename is possible only as an explicit identity migration,
not as an incidental repository or package edit.

## Preliminary collision search

The maintainer performed an exact-text, case-insensitive search for `Eqiora`
on 2026-07-23 across:

- GitHub repositories and general web-indexed software/company results;
- PyPI, crates.io, npm, RubyGems, and NuGet;
- the registered `eqiora.org` domain and its then-current DNS state;
- the public search surfaces of WIPO, UKIPO, USPTO, and EUIPO.

No relevant exact-name software distribution, engineering product, or company
was identified in that search, and the exact package names were not occupied
in the queried registries at that time. Search results are time-bound: package
indexes and marks may change after this decision, and absence of an exact hit
does not establish freedom to operate.

The public alpha deliberately uses the exact repository, distribution, import,
domain, version, and tag spellings in the decision table. Lookalike or
confusingly similar names are not treated as compatible aliases.

## Legal nonclaims

This RFC records a preliminary project-name search, not:

- legal advice or a legal opinion;
- trademark clearance, registrability, ownership, or priority;
- a search of every national register, unindexed business name, common-law
  use, product, or language;
- a conclusion about confusing similarity;
- a promise that a package registry or domain will remain available.

Before material commercial reliance or trademark filing, the maintainer
should obtain appropriate professional advice and repeat the relevant
jurisdictional searches.

## Migration rule

If a credible conflict appears, maintainers must first record its scope
privately when disclosure could prejudice a party, then publish an RFC that
defines:

1. the replacement project, domain, repository, distribution, and import
   identities;
2. package ownership and anti-confusion measures;
3. redirect, deprecation, and upgrade periods;
4. persisted-artifact and provenance treatment;
5. security communication and release-signing continuity.

Silent republishing under a lookalike name or reusing a released version is
not permitted.

## Nonclaims

The identity decision does not widen Eqiora's scientific capability,
stability, platform support, governance maturity, or safety status. Those
claims remain owned by the capability matrix, registered verification
evidence, release policy, and security policy.
