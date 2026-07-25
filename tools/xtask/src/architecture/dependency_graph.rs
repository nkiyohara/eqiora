//! Workspace dependency cycles.
//!
//! The layer check in `check-layers` already forbids an edge that points the
//! wrong way between declared layers. It cannot see a cycle that stays inside
//! one layer, and it ignores dev and build edges entirely — yet a dev-edge
//! cycle is what makes `cargo test -p` build the world, and it is the shape
//! that quietly reintroduces the coupling the layers exist to prevent.
//!
//! So this predicate is deliberately about the raw graph: every strongly
//! connected component of the workspace, over normal, build and dev edges
//! together, must be a single crate.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// The workspace dependency graph, crate name to the crates it depends on.
type Graph = BTreeMap<String, BTreeSet<String>>;

pub(super) struct Cycles {
    pub(super) packages: usize,
    pub(super) edges: usize,
    pub(super) violations: Vec<String>,
}

pub(super) fn check(root: &Path) -> Result<Cycles, String> {
    let graph = workspace_graph(root)?;
    let edges = graph.values().map(BTreeSet::len).sum();
    let violations = components(&graph)
        .into_iter()
        .filter(|component| component.len() > 1)
        .map(|component| {
            format!(
                "dependency cycle across {} crates: {}. Every workspace strongly connected \
                 component must have size 1 over normal, build and dev edges; break the cycle by \
                 moving the shared vocabulary into a lower crate.",
                component.len(),
                component.join(" -> ")
            )
        })
        .collect();

    Ok(Cycles {
        packages: graph.len(),
        edges,
        violations,
    })
}

/// `--no-deps` keeps registry packages out: a cycle through a third-party crate
/// is not something this repository can act on, and Cargo forbids it anyway.
/// The `dependencies` array of a member still lists its dev and build entries,
/// which is exactly the wider edge set the predicate wants.
fn workspace_graph(root: &Path) -> Result<Graph, String> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let metadata: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid cargo metadata: {error}"))?;
    let members: BTreeSet<&str> = metadata["workspace_members"]
        .as_array()
        .ok_or_else(|| "metadata has no workspace_members".to_owned())?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| "metadata has no packages".to_owned())?;

    let mut graph: Graph = BTreeMap::new();
    let mut names = BTreeSet::new();
    for package in packages {
        if package["id"]
            .as_str()
            .is_some_and(|id| members.contains(id))
            && let Some(name) = package["name"].as_str()
        {
            names.insert(name.to_owned());
        }
    }

    for package in packages {
        let Some(name) = package["name"].as_str().filter(|_| {
            package["id"]
                .as_str()
                .is_some_and(|id| members.contains(id))
        }) else {
            continue;
        };
        let dependencies = graph.entry(name.to_owned()).or_default();
        let Some(declared) = package["dependencies"].as_array() else {
            continue;
        };
        for dependency in declared {
            let Some(target) = dependency["name"].as_str() else {
                continue;
            };
            // A crate listing itself as a dev-dependency is a legal Cargo
            // idiom for integration tests, not a cycle between crates.
            if target != name && names.contains(target) {
                dependencies.insert(target.to_owned());
            }
        }
    }

    if graph.is_empty() {
        return Err("cargo metadata reported no workspace members".to_owned());
    }
    Ok(graph)
}

/// Tarjan's algorithm. Components come back in reverse topological order; each
/// is reported in the order the traversal entered it, which reads as a path
/// around the cycle rather than an unordered set.
fn components(graph: &Graph) -> Vec<Vec<String>> {
    let mut state = Tarjan {
        graph,
        index: BTreeMap::new(),
        low: BTreeMap::new(),
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        next: 0,
        components: Vec::new(),
    };
    for node in graph.keys() {
        if !state.index.contains_key(node.as_str()) {
            state.visit(node);
        }
    }
    state.components
}

struct Tarjan<'a> {
    graph: &'a Graph,
    index: BTreeMap<&'a str, usize>,
    low: BTreeMap<&'a str, usize>,
    stack: Vec<&'a str>,
    on_stack: BTreeSet<&'a str>,
    next: usize,
    components: Vec<Vec<String>>,
}

impl<'a> Tarjan<'a> {
    fn visit(&mut self, node: &'a str) {
        self.index.insert(node, self.next);
        self.low.insert(node, self.next);
        self.next += 1;
        self.stack.push(node);
        self.on_stack.insert(node);

        for target in self.graph.get(node).into_iter().flatten() {
            let target = target.as_str();
            // An edge to a crate outside the graph cannot close a cycle in it.
            let Some(key) = self
                .graph
                .get_key_value(target)
                .map(|(key, _)| key.as_str())
            else {
                continue;
            };
            let reachable = match self.index.get(key) {
                None => {
                    self.visit(key);
                    self.low[key]
                }
                Some(&seen) if self.on_stack.contains(key) => seen,
                Some(_) => continue,
            };
            let current = self.low[node];
            self.low.insert(node, current.min(reachable));
        }

        if self.low[node] == self.index[node] {
            self.close(node);
        }
    }

    fn close(&mut self, root: &'a str) {
        let mut component = Vec::new();
        while let Some(node) = self.stack.pop() {
            self.on_stack.remove(node);
            component.push(node.to_owned());
            if node == root {
                break;
            }
        }
        component.reverse();
        self.components.push(component);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(edges: &[(&str, &[&str])]) -> Graph {
        edges
            .iter()
            .map(|(node, targets)| {
                (
                    (*node).to_owned(),
                    targets.iter().map(|target| (*target).to_owned()).collect(),
                )
            })
            .collect()
    }

    fn cycles(graph: &Graph) -> Vec<Vec<String>> {
        let mut found: Vec<Vec<String>> = components(graph)
            .into_iter()
            .filter(|component| component.len() > 1)
            .collect();
        for component in &mut found {
            component.sort();
        }
        found.sort();
        found
    }

    #[test]
    fn an_acyclic_workspace_has_only_singleton_components() {
        let graph = graph(&[
            ("core", &[]),
            ("solver", &["core"]),
            ("numerics", &["core", "solver"]),
        ]);
        assert_eq!(components(&graph).len(), 3);
        assert!(cycles(&graph).is_empty());
    }

    #[test]
    fn a_three_crate_cycle_is_reported_as_one_component() {
        let graph = graph(&[
            ("a", &["b"]),
            ("b", &["c"]),
            ("c", &["a"]),
            ("d", &["a"]),
            ("e", &[]),
        ]);
        assert_eq!(cycles(&graph), [["a", "b", "c"]]);
    }

    #[test]
    fn a_two_crate_cycle_is_found_whichever_way_it_is_entered() {
        let graph = graph(&[("a", &["b"]), ("b", &["a"]), ("z", &["a", "b"])]);
        assert_eq!(cycles(&graph), [["a", "b"]]);
    }

    #[test]
    fn disjoint_cycles_are_reported_separately() {
        let graph = graph(&[
            ("a", &["b"]),
            ("b", &["a"]),
            ("c", &["d"]),
            ("d", &["c"]),
            ("bridge", &["a", "c"]),
        ]);
        assert_eq!(cycles(&graph), [["a", "b"], ["c", "d"]]);
    }

    #[test]
    fn an_edge_to_a_crate_outside_the_workspace_is_not_a_cycle() {
        let graph = graph(&[("a", &["serde", "b"]), ("b", &["serde"])]);
        assert!(cycles(&graph).is_empty());
    }
}
