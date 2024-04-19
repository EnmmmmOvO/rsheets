use crate::sheet::lock_pool::{get_or_insert, Sheet};
use rsheet_lib::cell_value::CellValue;
use rsheet_lib::cells::{column_name_to_number, column_number_to_name};
use rsheet_lib::command_runner::{CellArgument, CommandRunner};
use rsheet_lib::connect::ConnectionError;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub interval_time: u64,
    pub min: u32,
    pub max: u32,
    pub expansion_threshold: f32,
    pub expansion_multiplier: f32,
    pub contraction_threshold: f32,
    pub contraction_multiplier: f32,
}

pub fn set_cell_value(
    cell: String,
    formula: String,
    lock: Arc<RwLock<Sheet>>,
) -> Result<(), ConnectionError> {
    let time = SystemTime::now();
    let runner = CommandRunner::new(&formula);
    let (hash, mut dependency, check) = get_dependency_value(&runner, lock.clone());

    let value = if dependency.contains(&cell) {
        dependency.clear();
        CellValue::Error(format!("Cell {} is self-referential.", cell))
    } else if check {
        runner.run(&hash)
    } else {
        CellValue::Error("Reference a Error Cell.".to_string())
    };

    let cell_lock = get_or_insert(lock.clone(), &cell);
    let mut c = cell_lock.write().unwrap();
    c.set_value(formula.clone(), value, time, dependency, cell, lock.clone());

    Ok(())
}

pub fn get_dependency_value(
    runner: &CommandRunner,
    lock: Arc<RwLock<Sheet>>,
) -> (HashMap<String, CellArgument>, HashSet<String>, bool) {
    let mut hash = HashMap::new();
    let mut record = HashSet::new();
    let mut check = true;

    for i in runner.find_variables() {
        let regex =
            lazy_regex::regex_captures!(r"([A-Z]+)(\d+)(_([A-Z]+)(\d+))?", i.as_str()).unwrap();

        hash.insert(
            i.clone(),
            if regex.3.is_empty() {
                record.insert(i.clone());
                CellArgument::Value(get_check_cell_value(i, lock.clone(), &mut check))
            } else if regex.1 == regex.4 {
                let mut vec = vec![];
                for i in regex.2.parse::<u32>().unwrap()..=regex.5.parse::<u32>().unwrap() {
                    let temp = format!("{}{}", regex.1, i);
                    record.insert(temp.clone());
                    vec.push(get_check_cell_value(temp, lock.clone(), &mut check));
                }
                CellArgument::Vector(vec)
            } else if regex.2 == regex.5 {
                let mut vec = vec![];
                for i in column_name_to_number(regex.1)..=column_name_to_number(regex.4) {
                    let temp = format!("{}{}", column_number_to_name(i), regex.2);
                    record.insert(temp.clone());
                    vec.push(get_check_cell_value(temp, lock.clone(), &mut check));
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
                        let temp = format!("{}{}", column_number_to_name(j), i);
                        record.insert(temp.clone());
                        inner_vec.push(get_check_cell_value(temp, lock.clone(), &mut check));
                    }
                    vec.push(inner_vec);
                }

                CellArgument::Matrix(vec)
            },
        );
    }

    (hash, record, check)
}

pub fn get_check_cell_value(cell: String, lock: Arc<RwLock<Sheet>>, check: &mut bool) -> CellValue {
    let cell = get_cell_value(cell, lock.clone());
    if let CellValue::Error(_) = cell {
        *check = false;
    }
    cell
}

pub fn get_cell_value(cell: String, lock: Arc<RwLock<Sheet>>) -> CellValue {
    let cell_lock = get_or_insert(lock.clone(), &cell);

    let cell = cell_lock.read().unwrap();
    cell.get_value()
}
