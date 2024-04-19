use crate::sheet::{
    lib::get_dependency_value,
    lock_pool::{get_or_insert, unlock, LockPool},
};
use rsheet_lib::cell_value::CellValue;
use rsheet_lib::{
    cells::{column_name_to_number, column_number_to_name},
    command_runner::CommandRunner,
};
use std::collections::HashSet;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::spawn;
use std::time::SystemTime;

use crate::unlock;

#[derive(Debug, Clone)]
pub struct Cell {
    value: CellValue,
    formula: String,
    dependencies: HashSet<(u32, u32)>,
    timestamp: SystemTime,
}

impl Cell {
    pub fn new_blank() -> Cell {
        Cell {
            value: CellValue::None,
            formula: String::new(),
            dependencies: HashSet::new(),
            timestamp: SystemTime::UNIX_EPOCH,
        }
    }

    pub fn set_value(
        &mut self,
        formula: String,
        value: CellValue,
        time: SystemTime,
        dependency: HashSet<(u32, u32)>,
        (row, col): (u32, u32),
        (lock, condvar): (Arc<Mutex<LockPool>>, Arc<Condvar>),
    ) {
        if time <= self.timestamp {
            return;
        }

        let mut check = true;

        if let CellValue::Error(temp) = &self.value {
            if "Could not cast Rhai return back to Cell Value." == temp || temp.contains("is self-referential") {
                check = false;
            }
        }

        if check {
            relieve_dependencies(
                self.formula.clone(),
                row,
                col,
                lock.clone(),
                condvar.clone(),
            );
        }

        if CellValue::Error("Could not cast Rhai return back to Cell Value.".to_string()) == value {
            self.value = value;
        } else {
            check = false;

            for (r, c) in dependency {
                if r == row && c == col {
                    check = true;
                    continue;
                } else if self.dependencies.contains(&(r, c)) {
                    check = true;
                }
                let lock = lock.clone();
                let condvar = condvar.clone();

                spawn(move || {
                    add_dependencies(r, c, row, col, lock, condvar);
                });
            }

            self.value = if check {
                CellValue::Error(format!(
                    "Cell {}{} is self-referential",
                    column_number_to_name(col),
                    row + 1
                ))
            } else {
                value
            };
        }

        self.timestamp = time;
        self.formula = formula;

        for (r, c) in self.dependencies.clone() {
            let lock = lock.clone();
            let condvar = condvar.clone();
            spawn(move || {
                update_dependencies(r, c, lock, condvar);
            });
        }
    }

    pub fn update(&mut self, value: CellValue) {
        self.value = value;
    }

    pub fn get_value(&self) -> CellValue {
        self.value.clone()
    }

    pub fn add_dependency(&mut self, row: u32, col: u32) {
        self.dependencies.insert((row, col));
    }

    pub fn remove_dependency(&mut self, row: u32, col: u32) {
        self.dependencies.retain(|x| x != &(row, col));
    }

    pub fn get_formula(&self) -> &str {
        &self.formula
    }
}

pub fn add_dependencies(
    target_row: u32,
    target_col: u32,
    row: u32,
    col: u32,
    lock: Arc<Mutex<LockPool>>,
    condvar: Arc<Condvar>,
) {
    let cell_lock = get_or_insert(lock.clone(), condvar.clone(), target_row, target_col);
    let mut cell = cell_lock.lock().unwrap();
    cell.add_dependency(row, col);
    unlock!(lock, condvar, target_row, target_col, cell_lock, cell);
}

pub fn relieve_dependencies(
    formula: String,
    row: u32,
    col: u32,
    lock: Arc<Mutex<LockPool>>,
    condvar: Arc<Condvar>,
) {
    spawn(move || {
        for i in CommandRunner::new(&formula).find_variables() {
            let regex =
                lazy_regex::regex_captures!(r"([A-Z]+)(\d+)(_([A-Z]+)(\d+))?", i.as_str()).unwrap();

            if regex.3.is_empty() {
                let (target_row, target_col) = (
                    regex.2.parse::<u32>().unwrap() - 1,
                    column_name_to_number(regex.1),
                );
                remove_dependencies(
                    target_row,
                    target_col,
                    row,
                    col,
                    lock.clone(),
                    condvar.clone(),
                );
            } else if regex.1 == regex.4 {
                let target_col = column_name_to_number(regex.1);
                for i in regex.2.parse::<u32>().unwrap() - 1..=regex.5.parse::<u32>().unwrap() - 1 {
                    remove_dependencies(i, target_col, row, col, lock.clone(), condvar.clone());
                }
            } else if regex.2 == regex.5 {
                let target_row = regex.2.parse::<u32>().unwrap() - 1;
                for i in column_name_to_number(regex.1)..=column_name_to_number(regex.4) {
                    remove_dependencies(target_row, i, row, col, lock.clone(), condvar.clone());
                }
            } else {
                let (row1, col1) = (
                    regex.2.parse::<u32>().unwrap() - 1,
                    column_name_to_number(regex.1),
                );
                let (row2, col2) = (
                    regex.5.parse::<u32>().unwrap() - 1,
                    column_name_to_number(regex.4),
                );
                for i in row1..=row2 {
                    for j in col1..=col2 {
                        remove_dependencies(i, j, row, col, lock.clone(), condvar.clone());
                    }
                }
            }
        }
    });
}

pub fn remove_dependencies(
    target_row: u32,
    target_col: u32,
    row: u32,
    col: u32,
    lock: Arc<Mutex<LockPool>>,
    condvar: Arc<Condvar>,
) {
    let cell_lock = get_or_insert(lock.clone(), condvar.clone(), target_row, target_col);
    let mut cell = cell_lock.lock().unwrap();
    cell.remove_dependency(row, col);
    unlock!(lock, condvar, target_row, target_col, cell_lock, cell);
}

pub fn update_dependencies(row: u32, col: u32, lock: Arc<Mutex<LockPool>>, condvar: Arc<Condvar>) {
    let cell_lock = get_or_insert(lock.clone(), condvar.clone(), row, col);

    let binding = cell_lock.clone();
    let cell = binding.lock().unwrap();
    let runner = CommandRunner::new(cell.get_formula());
    drop(cell);
    drop(binding);

    let (hash, _) = get_dependency_value(&runner, lock.clone(), condvar.clone());
    let value = runner.run(&hash);
    let mut cell = cell_lock.lock().unwrap();
    cell.update(value);
    unlock!(lock, condvar, row, col, cell_lock, cell);
}
