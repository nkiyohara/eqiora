//! Deterministic finite search over a frozen, explicitly acquired inventory.

use super::*;

pub(super) type Requests = BTreeMap<QualifiedName, VersionRequest>;
pub(super) type Inventory = BTreeMap<PackageKey, Requests>;
pub(super) type Selection = BTreeMap<QualifiedName, PackageKey>;

const MAX_SEARCH_STEPS: usize = 100_000;
const MAX_SEARCH_WORK: usize = 2_000_000;
const MAX_REQUEST_EDGES: usize = 262_144;
const MAX_EXPLANATION_BYTES: usize = 16 * 1024 * 1024;

struct Decision {
    name: QualifiedName,
    chosen: ExactVersion,
    remaining: std::vec::IntoIter<PackageKey>,
}

#[derive(Clone)]
struct Requirement {
    request: VersionRequest,
    path: String,
}

enum Failure {
    Conflict(PackagePreparationError),
    Exhausted(PackagePreparationError),
}

impl From<PackagePreparationError> for Failure {
    fn from(error: PackagePreparationError) -> Self {
        Self::Conflict(error)
    }
}

/// No IO, compilation or cache access is permitted during selection.
pub(super) fn solve(
    root: &PackageKey,
    inventory: &Inventory,
    locked: Option<&BTreeMap<QualifiedName, ExactVersion>>,
) -> Result<Selection, PackagePreparationError> {
    solve_with_budget(root, inventory, locked, MAX_SEARCH_WORK)
}

fn solve_with_budget(
    root: &PackageKey,
    inventory: &Inventory,
    locked: Option<&BTreeMap<QualifiedName, ExactVersion>>,
    mut remaining_work: usize,
) -> Result<Selection, PackagePreparationError> {
    let mut selected = BTreeMap::from([(root.0.clone(), root.clone())]);
    let mut decisions: Vec<Decision> = Vec::new();
    let mut solution: Option<Selection> = None;
    let mut competing_depth: Option<usize> = None;
    for _ in 0..MAX_SEARCH_STEPS {
        // Charge the full scans before performing them. Each request is bounded;
        // repeated backtracking cannot turn the finite inventory into unbounded work.
        let work = inventory.len().saturating_add(
            selected
                .values()
                .map(|key| inventory.get(key).map_or(0, BTreeMap::len))
                .sum::<usize>(),
        );
        remaining_work = remaining_work.checked_sub(work).ok_or_else(|| {
            git::error("version selection exceeds the bounded search work budget")
        })?;
        let step = next(root, inventory, &selected, locked);
        let failure = match step {
            Ok(None) => {
                if let Some(previous) = &solution {
                    let name = selected
                        .iter()
                        .find_map(|(name, key)| {
                            previous
                                .get(name)
                                .filter(|prior| {
                                    *prior != key && prior.1.precedence_cmp(&key.1).is_eq()
                                })
                                .map(|_| name.as_str())
                        })
                        .unwrap_or("complete selection");
                    return Err(git::error(&format!(
                        "ambiguous equal-precedence releases for `{name}` admit multiple complete selections; use an exact request including build metadata"
                    )));
                }
                solution = Some(selected.clone());
                git::error("complete selection")
            }
            Ok(Some((name, choices))) => {
                let mut remaining = choices.into_iter();
                let choice = remaining.next().expect("nonempty admitted choices");
                let chosen = choice.1.clone();
                selected.insert(name.clone(), choice);
                decisions.push(Decision {
                    name,
                    chosen,
                    remaining,
                });
                continue;
            }
            Err(Failure::Exhausted(error)) => return Err(error),
            Err(Failure::Conflict(error)) => error,
        };
        // Retract every dependent decision, not only the last failed request.
        loop {
            let depth = decisions.len();
            let Some(decision) = decisions.last_mut() else {
                return solution.ok_or(failure);
            };
            selected.remove(&decision.name);
            if let Some(choice) = decision.remaining.next() {
                // Once a complete solution exists, only equal-precedence branches
                // may compete. Their iteration order never selects an identity.
                if solution.is_none()
                    || competing_depth.is_some_and(|ancestor| depth > ancestor)
                    || choice.1.precedence_cmp(&decision.chosen).is_eq()
                {
                    if solution.is_some() && competing_depth.is_none() {
                        competing_depth = Some(depth);
                    }
                    decision.chosen = choice.1.clone();
                    selected.insert(decision.name.clone(), choice);
                    break;
                }
            }
            if competing_depth == Some(depth) {
                competing_depth = None;
            }
            decisions.pop();
        }
    }
    Err(git::error(
        "version selection exceeds the bounded search budget",
    ))
}

fn next(
    root: &PackageKey,
    inventory: &Inventory,
    selected: &Selection,
    locked: Option<&BTreeMap<QualifiedName, ExactVersion>>,
) -> Result<Option<(QualifiedName, Vec<PackageKey>)>, Failure> {
    let requirements = requirements(root, inventory, selected)?;
    for (name, requests) in &requirements {
        if let Some(key) = selected.get(name)
            && requests
                .iter()
                .any(|required| !required.request.matches(&key.1))
        {
            return Err(conflict(name, requests).into());
        }
    }
    let Some((name, requests)) = requirements
        .iter()
        .find(|(name, _)| !selected.contains_key(*name))
    else {
        require_acyclic(inventory, selected)?;
        return Ok(None);
    };
    let mut choices = inventory
        .keys()
        .filter(|key| {
            &key.0 == name
                && requests
                    .iter()
                    .all(|required| required.request.matches(&key.1))
                && locked.is_none_or(|locked| locked.get(name) == Some(&key.1))
        })
        .cloned()
        .collect::<Vec<_>>();
    choices.sort_by(|left, right| right.1.precedence_cmp(&left.1));
    if choices.is_empty() {
        return Err(conflict(name, requests).into());
    }
    Ok(Some((name.clone(), choices)))
}

fn require_acyclic(
    inventory: &Inventory,
    selected: &Selection,
) -> Result<(), PackagePreparationError> {
    let mut incoming = selected
        .keys()
        .map(|name| (name.clone(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    for key in selected.values() {
        for name in inventory.get(key).expect("selected requests").keys() {
            *incoming.get_mut(name).expect("complete dependency") += 1;
        }
    }
    let mut pending = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let mut visited = 0;
    while let Some(name) = pending.pop() {
        visited += 1;
        for dependency in inventory
            .get(&selected[&name])
            .expect("selected requests")
            .keys()
        {
            let count = incoming.get_mut(dependency).expect("complete dependency");
            *count -= 1;
            if *count == 0 {
                pending.push(dependency.clone());
            }
        }
    }
    if visited != selected.len() {
        return Err(git::error("version dependency cycle in complete selection"));
    }
    Ok(())
}

fn requirements(
    root: &PackageKey,
    inventory: &Inventory,
    selected: &Selection,
) -> Result<BTreeMap<QualifiedName, Vec<Requirement>>, Failure> {
    let mut result: BTreeMap<_, Vec<_>> = BTreeMap::new();
    let mut pending = std::collections::VecDeque::from([(root.clone(), vec![root.0.clone()])]);
    let mut expanded = BTreeSet::new();
    let mut edges = 0;
    let mut explanation_bytes = 0_usize;
    while let Some((key, path)) = pending.pop_front() {
        if !expanded.insert(key.clone()) {
            continue;
        }
        let requests = inventory
            .get(&key)
            .ok_or_else(|| git::error("selected package is absent from frozen inventory"))?;
        for (name, request) in requests {
            edges += 1;
            if edges > MAX_REQUEST_EDGES || path.len() > MAX_LOCAL_DEPENDENCY_DEPTH {
                return Err(Failure::Exhausted(git::error(
                    "version dependency paths exceed their resource bounds",
                )));
            }
            let mut child_path = path.clone();
            child_path.push(name.clone());
            let rendered = child_path
                .iter()
                .map(QualifiedName::as_str)
                .collect::<Vec<_>>()
                .join(" -> ");
            explanation_bytes += rendered.len() + request.as_str().len();
            if explanation_bytes > MAX_EXPLANATION_BYTES {
                return Err(Failure::Exhausted(git::error(
                    "version dependency explanations exceed their byte bound",
                )));
            }
            result.entry(name.clone()).or_default().push(Requirement {
                request: request.clone(),
                path: rendered.clone(),
            });
            if let Some(child) = selected.get(name) {
                if path.contains(name) {
                    return Err(git::error(&format!("version dependency cycle: {rendered}")).into());
                }
                pending.push_back((child.clone(), child_path));
            }
        }
    }
    Ok(result)
}

fn conflict(name: &QualifiedName, requests: &[Requirement]) -> PackagePreparationError {
    let explanation = requests
        .iter()
        .map(|required| format!("{} requests {name}@{}", required.path, required.request))
        .collect::<Vec<_>>()
        .join("; ");
    git::error(&format!(
        "no single release satisfies `{name}`: {explanation}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str, version: &str) -> PackageKey {
        (
            QualifiedName::parse(name).unwrap(),
            ExactVersion::parse(version).unwrap(),
        )
    }
    fn requests(values: &[(&str, &str)]) -> Requests {
        values
            .iter()
            .map(|(name, request)| {
                (
                    QualifiedName::parse(*name).unwrap(),
                    VersionRequest::parse(*request).unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn complete_global_search_backtracks_and_is_inventory_order_independent() {
        // Highest A needs C@2, but B requires C@1. A@1.9 is the first complete solution.
        let entries = vec![
            (key("Root", "1.0.0"), requests(&[("A", "1"), ("B", "1")])),
            (key("A", "1.10.0"), requests(&[("C", "2")])),
            (key("A", "1.9.0"), requests(&[("C", "1")])),
            (key("B", "1.0.0"), requests(&[("C", "1")])),
            (key("C", "1.0.0"), requests(&[])),
            (key("C", "2.0.0"), requests(&[])),
        ];
        let root = key("Root", "1.0.0");
        let inventory = entries.clone().into_iter().collect();
        let selected = solve(&root, &inventory, None).unwrap();
        assert_eq!(selected[&root.0], root);
        assert_eq!(selected[&key("A", "1.9.0").0], key("A", "1.9.0"));
        assert_eq!(selected[&key("C", "1.0.0").0], key("C", "1.0.0"));
        assert_eq!(selected.len(), 4);
        let reversed = entries.into_iter().rev().collect();
        assert_eq!(selected, solve(&root, &reversed, None).unwrap());
    }

    #[test]
    fn conflicts_include_both_requests_and_dependency_paths() {
        let root = key("Root", "1.0.0");
        let inventory = BTreeMap::from([
            (root.clone(), requests(&[("A", "1"), ("C", "1")])),
            (key("A", "1.0.0"), requests(&[("C", "2")])),
            (key("C", "1.0.0"), requests(&[])),
            (key("C", "2.0.0"), requests(&[])),
        ]);
        let error = solve(&root, &inventory, None).unwrap_err().to_string();
        assert!(error.contains("Root -> C requests C@1"), "{error}");
        assert!(error.contains("Root -> A -> C requests C@2"), "{error}");
    }

    #[test]
    fn equal_precedence_is_not_resolved_by_build_text_and_lock_never_advances() {
        let root = key("Root", "1.0.0");
        let mut inventory = BTreeMap::from([
            (root.clone(), requests(&[("A", "1")])),
            (key("A", "1.0.0+first"), requests(&[])),
            (key("A", "1.0.0+second"), requests(&[])),
        ]);
        assert!(
            solve(&root, &inventory, None)
                .unwrap_err()
                .to_string()
                .contains("equal-precedence")
        );
        inventory.insert(root.clone(), requests(&[("A", "1.0.0+second")]));
        assert_eq!(
            solve(&root, &inventory, None).unwrap()[&key("A", "1.0.0").0],
            key("A", "1.0.0+second")
        );
        inventory.insert(root.clone(), requests(&[("A", "1")]));
        let locked = BTreeMap::from([(
            key("A", "1.0.0").0,
            ExactVersion::parse("1.0.0+first").unwrap(),
        )]);
        assert_eq!(
            solve(&root, &inventory, Some(&locked)).unwrap()[&key("A", "1.0.0").0],
            key("A", "1.0.0+first")
        );
    }

    #[test]
    fn transitive_exact_request_resolves_a_precedence_tie_and_cycles_backtrack() {
        let root = key("Root", "1.0.0");
        let mut inventory = BTreeMap::from([
            (root.clone(), requests(&[("A", "1"), ("B", "1")])),
            (key("A", "1.0.0+first"), requests(&[])),
            (key("A", "1.0.0+second"), requests(&[])),
            (key("B", "1.0.0"), requests(&[("A", "1.0.0+second")])),
        ]);
        let selected = solve(&root, &inventory, None).unwrap();
        assert_eq!(selected[&key("A", "1.0.0").0], key("A", "1.0.0+second"));
        inventory.insert(key("B", "1.1.0"), requests(&[("C", "1")]));
        inventory.insert(key("C", "1.0.0"), requests(&[("B", "1")]));
        assert_eq!(solve(&root, &inventory, None).unwrap(), selected);
    }

    #[test]
    fn budget_exhaustion_is_not_reported_as_a_conflict_or_an_older_solution() {
        let root = key("Root", "1.0.0");
        let inventory = BTreeMap::from([
            (root.clone(), requests(&[("A", "1")])),
            (key("A", "1.0.0"), requests(&[])),
            (key("A", "1.1.0"), requests(&[])),
        ]);
        assert_eq!(
            solve(&root, &inventory, None).unwrap()[&key("A", "1.0.0").0],
            key("A", "1.1.0")
        );
        let error = solve_with_budget(&root, &inventory, None, 1)
            .unwrap_err()
            .to_string();
        assert!(error.contains("search work budget"));
        assert!(!error.contains("no single release"));
    }

    #[test]
    fn equal_precedence_ancestor_receives_complete_downstream_backtracking() {
        let root = key("Root", "1.0.0");
        let inventory = BTreeMap::from([
            (
                root.clone(),
                requests(&[("A", "1"), ("B", "1"), ("C", "1")]),
            ),
            (key("A", "1.0.0+first"), requests(&[("C", "1.1.0")])),
            (key("A", "1.0.0+second"), requests(&[("C", "1.0.0")])),
            (key("B", "1.1.0"), requests(&[("C", "1.1.0")])),
            (key("B", "1.0.0"), requests(&[("C", "1.0.0")])),
            (key("C", "1.1.0"), requests(&[])),
            (key("C", "1.0.0"), requests(&[])),
        ]);
        assert!(
            solve(&root, &inventory, None)
                .unwrap_err()
                .to_string()
                .contains("equal-precedence")
        );
    }
}
