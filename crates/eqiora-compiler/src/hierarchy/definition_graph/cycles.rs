//! Iterative cycle discovery and cycle diagnostics for reusable definitions.

use super::*;

pub(super) fn strongly_connected_components(
    nodes: &[ComponentNode<'_>],
) -> Result<DefinitionOrder, Vec<Diagnostic>> {
    let mut visited = vec![false; nodes.len()];
    let mut finish = Vec::new();
    finish
        .try_reserve_exact(nodes.len())
        .map_err(|_| vec![definition_error("cannot reserve definition finish order")])?;
    for start in 0..nodes.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut stack = vec![(start, 0_usize)];
        while let Some((node, edge_index)) = stack.last_mut() {
            if *edge_index < nodes[*node].edges.len() {
                let target = nodes[*node].edges[*edge_index].target;
                *edge_index += 1;
                if !visited[target] {
                    visited[target] = true;
                    stack.push((target, 0));
                }
            } else {
                let (node, _) = stack.pop().expect("definition DFS frame exists");
                finish.push(node);
            }
        }
    }

    let mut reverse = vec![Vec::new(); nodes.len()];
    let mut incoming = vec![0_usize; nodes.len()];
    for node in nodes {
        for edge in &node.edges {
            incoming[edge.target] = incoming[edge.target].checked_add(1).ok_or_else(|| {
                vec![definition_error(
                    "reverse definition-edge count overflows usize",
                )]
            })?;
        }
    }
    for (edges, capacity) in reverse.iter_mut().zip(incoming) {
        edges
            .try_reserve_exact(capacity)
            .map_err(|_| vec![definition_error("cannot reserve reverse definition graph")])?;
    }
    for (source, node) in nodes.iter().enumerate() {
        for edge in &node.edges {
            reverse[edge.target].push(source);
        }
    }
    for parents in &mut reverse {
        parents.sort_unstable();
    }

    visited.fill(false);
    let mut cyclic = Vec::new();
    for &start in finish.iter().rev() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut component = Vec::new();
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            component.push(node);
            for &parent in reverse[node].iter().rev() {
                if !visited[parent] {
                    visited[parent] = true;
                    stack.push(parent);
                }
            }
        }
        component.sort_unstable();
        let self_loop = component.len() == 1
            && nodes[component[0]]
                .edges
                .iter()
                .any(|edge| edge.target == component[0]);
        if component.len() > 1 || self_loop {
            cyclic.push(component);
        }
    }
    cyclic.sort_by_key(|component| component[0]);
    Ok(DefinitionOrder {
        children_first: finish,
        cyclic_components: cyclic,
    })
}

pub(super) fn cycle_diagnostic(nodes: &[ComponentNode<'_>], component: &[usize]) -> Diagnostic {
    let start = component[0];
    let members = component.iter().copied().collect::<BTreeSet<_>>();
    let first_edge = nodes[start]
        .edges
        .iter()
        .find(|edge| members.contains(&edge.target))
        .expect("cyclic SCC has an internal edge from every member");
    let mut path = vec![start];
    if first_edge.target == start {
        path.push(start);
    } else {
        path.push(first_edge.target);
        let mut queue = VecDeque::from([first_edge.target]);
        let mut predecessor = BTreeMap::<usize, usize>::new();
        predecessor.insert(first_edge.target, first_edge.target);
        while let Some(node) = queue.pop_front() {
            if node == start {
                break;
            }
            for edge in &nodes[node].edges {
                if members.contains(&edge.target) && !predecessor.contains_key(&edge.target) {
                    predecessor.insert(edge.target, node);
                    queue.push_back(edge.target);
                }
            }
        }
        let mut suffix = vec![start];
        let mut cursor = start;
        while cursor != first_edge.target {
            cursor = predecessor[&cursor];
            suffix.push(cursor);
        }
        suffix.reverse();
        path.extend(suffix.into_iter().skip(1));
    }
    let display = path
        .into_iter()
        .map(|node| nodes[node].key.display())
        .collect::<Vec<_>>()
        .join(" -> ");
    source_error(
        codes::LANGUAGE_TYPE_ERROR,
        first_edge.file,
        first_edge.range,
        format!("recursive component definition graph: {display}"),
    )
}
