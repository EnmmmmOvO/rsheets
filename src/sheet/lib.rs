use crate::sheet::sheet_pool::{get_or_insert, Sheet};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{Dfs, VisitMap, Visitable};
use rsheet_lib::cell_value::CellValue;
use rsheet_lib::cells::{column_name_to_number, column_number_to_name};
use rsheet_lib::command_runner::{CellArgument, CommandRunner};
use rsheet_lib::connect::ConnectionError;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

macro_rules! get_check_cell_value {
    ($cell:expr, $lock:expr, $check:expr, $graph:expr) => {
        match get_cell_value_and_node($cell, $lock.clone(), $graph.clone()) {
            (CellValue::Error(e), node) => {
                if e.contains("is self-referential") {
                    $check = 1;
                } else {
                    $check = 2;
                }
                (CellValue::Error(e), node)
            }
            value => value,
        }
    };
}

pub fn set_cell_value(
    cell: String,
    formula: String,
    sheet: Arc<RwLock<Sheet>>,
    graph: Arc<RwLock<DiGraph<String, ()>>>,
) -> Result<(), ConnectionError> {
    let time = SystemTime::now();
    let runner = CommandRunner::new(&formula);
    let (value, dependency) = get_dependency_value(runner, &cell, sheet.clone(), graph.clone());

    let cell_lock = get_or_insert(sheet.clone(), &cell, graph.clone());
    let mut c = cell_lock.write().unwrap();
    c.set_value(
        formula.clone(),
        value,
        time,
        dependency,
        sheet.clone(),
        graph.clone(),
    );

    Ok(())
}

pub fn get_dependency_value(
    runner: CommandRunner,
    cell: &str,
    lock: Arc<RwLock<Sheet>>,
    graph: Arc<RwLock<DiGraph<String, ()>>>,
) -> (CellValue, HashSet<NodeIndex>) {
    let mut hash = HashMap::new();
    let mut record = HashSet::new();
    let mut check = 0;

    for i in runner.find_variables() {
        let regex =
            lazy_regex::regex_captures!(r"([A-Z]+)(\d+)(_([A-Z]+)(\d+))?", i.as_str()).unwrap();

        hash.insert(
            i.clone(),
            if regex.3.is_empty() {
                let (cell, node) = get_check_cell_value!(i, lock.clone(), check, graph.clone());
                record.insert(node);
                CellArgument::Value(cell)
            } else if regex.1 == regex.4 {
                let mut vec = vec![];
                for i in regex.2.parse::<u32>().unwrap()..=regex.5.parse::<u32>().unwrap() {
                    let (cell, node) = get_check_cell_value!(
                        format!("{}{}", regex.1, i),
                        lock.clone(),
                        check,
                        graph.clone()
                    );
                    record.insert(node);
                    if check != 0 {
                        continue;
                    }
                    vec.push(cell);
                }
                CellArgument::Vector(vec)
            } else if regex.2 == regex.5 {
                let mut vec = vec![];
                for i in column_name_to_number(regex.1)..=column_name_to_number(regex.4) {
                    let (cell, node) = get_check_cell_value!(
                        format!("{}{}", column_number_to_name(i), regex.2),
                        lock.clone(),
                        check,
                        graph.clone()
                    );
                    record.insert(node);
                    if check != 0 {
                        continue;
                    }
                    vec.push(cell);
                }
                CellArgument::Vector(vec)
            } else {
                let (row1, col1) = (
                    regex.2.parse::<u32>().unwrap(),
                    column_name_to_number(regex.1),
                );
                let (row2, col2) = (
                    regex.5.parse::<u32>().unwrap(),
                    column_name_to_number(regex.4),
                );
                let mut vec = vec![];

                for i in row1..=row2 {
                    let mut inner_vec = vec![];
                    for j in col1..=col2 {
                        let (cell, node) = get_check_cell_value!(
                            format!("{}{}", column_number_to_name(j), i),
                            lock.clone(),
                            check,
                            graph.clone()
                        );
                        record.insert(node);
                        if check != 0 {
                            continue;
                        }
                        inner_vec.push(cell);
                    }
                    vec.push(inner_vec);
                }

                CellArgument::Matrix(vec)
            },
        );
    }

    (
        match check {
            0 => runner.run(&hash),
            1 => CellValue::Error(format!("Cell {cell} is self-referential")),
            _ => CellValue::Error("Reference a Error Cell.".to_string()),
        },
        record,
    )
}

pub fn get_cell_value_and_node(
    cell: String,
    lock: Arc<RwLock<Sheet>>,
    graph: Arc<RwLock<DiGraph<String, ()>>>,
) -> (CellValue, NodeIndex) {
    let cell_lock = get_or_insert(lock.clone(), &cell, graph);

    let cell = cell_lock.read().unwrap();
    cell.get_value_and_node()
}

pub fn get_cell_value(
    cell: String,
    lock: Arc<RwLock<Sheet>>,
    graph: Arc<RwLock<DiGraph<String, ()>>>,
) -> CellValue {
    let cell_lock = get_or_insert(lock.clone(), &cell, graph);

    let cell = cell_lock.read().unwrap();
    cell.get_value()
}

pub fn dfs_cycle_detect(graph: Arc<RwLock<DiGraph<String, ()>>>, start_node: NodeIndex) -> bool {
    let graph = graph.read().unwrap();
    let g = graph.deref();
    let mut dfs = Dfs::new(&g, start_node);
    let mut visited = graph.visit_map();

    while let Some(nx) = dfs.next(&g) {
        if dfs.stack.contains(&nx) {
            return true;
        }
        visited.visit(nx);
    }

    false
}
