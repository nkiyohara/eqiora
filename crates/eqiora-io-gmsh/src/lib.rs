//! Bounded ASCII and binary Gmsh MSH 4.1 adapters into Eqiora mesh contracts.
//!
//! Gmsh syntax is intentionally confined to this L3 crate. Accepted input is
//! reconstructed through [`eqiora_meshing::SimplicialMesh`], so importer
//! success never bypasses the topology, geometry, orientation, or quality
//! invariants used by numerical assembly. Caller-owned semantic and aggregate
//! decoded-resource limits, remaining-byte lower bounds, and fallible
//! declaration-sized reservations close the decoder boundary before dependency
//! parsing.

mod msh41;

pub use msh41::{GmshImportLimits, GmshSimplexImporter};
