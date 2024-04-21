use crate::sheet::lib::get_dependency_value;
use crate::sheet::sheet_pool::get_or_insert;
use crate::sheet::{lib::dfs_cycle_detect, sheet_pool::Sheet};
use petgraph::graph::{DiGraph, NodeIndex};
use rsheet_lib::cell_value::CellValue;
use rsheet_lib::command_runner::CommandRunner;
use std::collections::HashSet;
use std::ops::DerefMut;
use std::sync::{Arc, RwLock};
use std::thread::spawn;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct Cell {
    value: CellValue,
    formula: String,
    node: NodeIndex,
    cell: String,
    timestamp: SystemTime,
}

impl Cell {
    pub fn new_blank(node: NodeIndex, cell: String) -> Cell {
        Cell {
            value: CellValue::None,
            formula: String::new(),
            node,
            cell,
            timestamp: SystemTime::UNIX_EPOCH,
        }
    }

    pub fn get_formula(&self) -> &str {
        &self.formula
    }

    pub fn set_value(
        &mut self,
        formula: String,
        value: CellValue,
        time: SystemTime,
        dependency: HashSet<NodeIndex>,
        sheet: Arc<RwLock<Sheet>>,
        graph: Arc<RwLock<DiGraph<String, ()>>>,
    ) {
        if time <= self.timestamp {
            return;
        }

        let mut check = true;

        if let CellValue::Error(temp) = &self.value {
            if "Could not cast Rhai return back to Cell Value." == temp {
                check = false;
            }
        }

        self.formula = formula;
        self.timestamp = time;

        let graph_lock = graph.clone();

        if check {
            let mut g = graph_lock.write().unwrap();
            let g_mut = g.deref_mut();
            g_mut.retain_edges(|graph, edge| graph.edge_endpoints(edge).unwrap().1 != self.node);
            drop(g);
        }

        check = false;

        if CellValue::Error("Could not cast Rhai return back to Cell Value.".to_string()) == value {
            self.value = value;
        } else {
            let mut g = graph_lock.write().unwrap();
            let g_mut = g.deref_mut();
            for i in dependency {
                if i == self.node {
                    check = true;
                    continue;
                }

                g_mut.add_edge(i, self.node, ());
            }
            drop(g);

            check = check || dfs_cycle_detect(graph.clone(), self.node);

            self.value = if check {
                CellValue::Error(format!("Cell {} is self-referential", self.cell))
            } else {
                value
            };
        }

        let node = self.node;
        spawn(move || {
            let mut set = HashSet::new();
            set.insert(node);
            dfs_recursive(graph_lock, node, &mut set, check, sheet);
        });
    }

    pub fn update(&mut self, value: CellValue) {
        self.value = value;
    }

    pub fn update_err(&mut self) {
        self.value = CellValue::Error(format!("Cell {} is self-referential", self.cell));
    }

    pub fn get_value(&self) -> CellValue {
        self.value.clone()
    }

    pub fn get_value_and_node(&self) -> (CellValue, NodeIndex) {
        (self.value.clone(), self.node)
    }
}

pub fn dfs_recursive(
    graph_lock: Arc<RwLock<DiGraph<String, ()>>>,
    node_index: NodeIndex,
    visited: &mut HashSet<NodeIndex>,
    err: bool,
    sheet: Arc<RwLock<Sheet>>,
) {
    let binding = graph_lock.clone();
    let graph = binding.read().unwrap();
    let temp = graph
        .neighbors(node_index)
        .map(|x| (x, graph.node_weight(x).unwrap().to_string()))
        .collect::<Vec<_>>();
    drop(graph);

    for (neighbor, cell) in temp {
        let sheet = sheet.clone();
        if !visited.contains(&neighbor) {
            visited.insert(neighbor);
            let graph_temp = graph_lock.clone();
            let sheet_temp = sheet.clone();
            let err_temp = err;
            spawn(move || {
                update_dependencies(sheet_temp, cell, graph_temp, err_temp);
            });
            dfs_recursive(graph_lock.clone(), neighbor, visited, err, sheet);
        }
    }
}

pub fn update_dependencies(
    lock: Arc<RwLock<Sheet>>,
    cell: String,
    graph: Arc<RwLock<DiGraph<String, ()>>>,
    err: bool,
) {
    let cell_lock = get_or_insert(lock.clone(), &cell, graph.clone());

    if err {
        let mut cell = cell_lock.write().unwrap();
        cell.update_err();
        return;
    }

    let binding = cell_lock.clone();
    let c = binding.read().unwrap();
    let runner = CommandRunner::new(c.get_formula());
    drop(c);

    let (value, _) = get_dependency_value(runner, &cell, lock.clone(), graph.clone());

    let mut cell = cell_lock.write().unwrap();
    cell.update(value);
}
