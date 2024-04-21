use crate::sheet::cell::update_dependencies;
use crate::sheet::{Cell, Sheet};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{VisitMap, Visitable};
use std::collections::HashSet;
use std::ops::Deref;
use std::sync::{Arc, RwLock};
use std::thread::spawn;

pub fn dfs_cycle_detect(
    graph: Arc<RwLock<DiGraph<Arc<RwLock<Cell>>, ()>>>,
    start_node: NodeIndex,
) -> bool {
    let graph = graph.read().unwrap();
    let g = graph.deref();
    let mut visited = g.visit_map();
    let mut recursion_stack = vec![];

    has_cycle(g, start_node, &mut visited, &mut recursion_stack)
}

fn has_cycle(
    graph: &DiGraph<Arc<RwLock<Cell>>, ()>,
    node: NodeIndex,
    visited: &mut dyn VisitMap<NodeIndex>,
    recursion_stack: &mut Vec<NodeIndex>,
) -> bool {
    visited.visit(node);
    recursion_stack.push(node);

    for neighbor in graph.neighbors(node) {
        if !visited.is_visited(&neighbor) {
            if has_cycle(graph, neighbor, visited, recursion_stack) {
                return true;
            }
        } else if recursion_stack.contains(&neighbor) {
            return true;
        }
    }

    recursion_stack.pop();
    false
}

pub fn dfs_recursive(
    graph_lock: Arc<RwLock<DiGraph<Arc<RwLock<Cell>>, ()>>>,
    node_index: NodeIndex,
    sheet: Arc<RwLock<Sheet>>,
) {
    let binding = graph_lock.clone();
    let graph = binding.read().unwrap();
    let temp = graph
        .neighbors(node_index)
        .map(|x| (x, graph.node_weight(x).unwrap().clone()))
        .collect::<Vec<_>>();
    drop(graph);

    for (neighbor, cell) in temp {
        let sheet_temp = sheet.clone();
        let graph_temp = graph_lock.clone();
        spawn(move || {
            dfs_recursive(graph_temp.clone(), neighbor, sheet_temp.clone());
            update_dependencies(cell, sheet_temp, graph_temp);
        });
    }
}

pub fn dfs_cycle_recursive(
    graph_lock: Arc<RwLock<DiGraph<Arc<RwLock<Cell>>, ()>>>,
    node_index: NodeIndex,
    visited: &mut HashSet<NodeIndex>,
    sheet: Arc<RwLock<Sheet>>,
) {
    let binding = graph_lock.clone();
    let graph = binding.read().unwrap();
    let temp = graph
        .neighbors(node_index)
        .map(|x| (x, graph.node_weight(x).unwrap().clone()))
        .collect::<Vec<_>>();
    drop(graph);

    for (neighbor, cell) in temp {
        if visited.contains(&neighbor) {
            continue;
        }
        visited.insert(neighbor);
        let sheet = sheet.clone();
        spawn(move || {
            let mut c = cell.write().unwrap();
            c.update_err();
        });
        dfs_cycle_recursive(graph_lock.clone(), neighbor, visited, sheet);
    }
}
