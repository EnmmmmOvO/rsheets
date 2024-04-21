use crate::sheet::graph::{dfs_cycle_recursive, dfs_recursive};
use crate::sheet::lib::get_dependency_value;
use crate::sheet::{graph::dfs_cycle_detect, sheet_pool::Sheet};
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
    node: Option<NodeIndex>,
    cell: String,
    timestamp: SystemTime,
}

impl Cell {
    pub fn new_blank(cell: String) -> Cell {
        Cell {
            value: CellValue::None,
            formula: String::new(),
            node: None,
            cell,
            timestamp: SystemTime::UNIX_EPOCH,
        }
    }

    pub fn set_node(&mut self, node: NodeIndex) {
        self.node = Some(node);
    }

    pub fn get_formula_and_cell(&self) -> (String, String) {
        (self.formula.to_string(), self.cell.to_string())
    }

    pub fn set_value(
        &mut self,
        formula: String,
        value: CellValue,
        time: SystemTime,
        dependency: HashSet<NodeIndex>,
        sheet: Arc<RwLock<Sheet>>,
        graph: Arc<RwLock<DiGraph<Arc<RwLock<Cell>>, ()>>>,
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
            g_mut.retain_edges(|graph, edge| {
                graph.edge_endpoints(edge).unwrap().1 != self.node.unwrap()
            });
            drop(g);
        }

        check = false;

        if CellValue::Error("Could not cast Rhai return back to Cell Value.".to_string()) == value {
            self.value = value;
        } else {
            let mut g = graph_lock.write().unwrap();
            let g_mut = g.deref_mut();
            for i in dependency {
                if i == self.node.unwrap() {
                    check = true;
                    continue;
                }

                g_mut.add_edge(i, self.node.unwrap(), ());
            }
            drop(g);

            check = check || dfs_cycle_detect(graph.clone(), self.node.unwrap());

            self.value = if check {
                CellValue::Error(format!("Cell {} is self-referential", self.cell))
            } else {
                value
            };
        }

        let node = self.node.unwrap();
        if check {
            let mut set = HashSet::new();
            set.insert(node);
            spawn(move || {
                dfs_cycle_recursive(graph_lock, node, &mut set, sheet);
            });
        } else {
            spawn(move || {
                dfs_recursive(graph_lock, node, sheet);
            });
        }
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
        (self.value.clone(), self.node.unwrap())
    }
}

pub fn update_dependencies(
    cell_lock: Arc<RwLock<Cell>>,
    lock: Arc<RwLock<Sheet>>,
    graph: Arc<RwLock<DiGraph<Arc<RwLock<Cell>>, ()>>>,
) {
    let binding = cell_lock.clone();
    let c = binding.read().unwrap();
    let (formula, cell) = c.get_formula_and_cell();
    drop(c);

    let runner = CommandRunner::new(&formula);

    let (value, _) = get_dependency_value(runner, &cell, lock.clone(), graph.clone());

    let mut cell = cell_lock.write().unwrap();
    cell.update(value);
}
