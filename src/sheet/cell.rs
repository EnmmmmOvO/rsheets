use crate::sheet::{
    lib::get_dependency_value,
    lock_pool::{get_or_insert, Sheet},
};
use rsheet_lib::cell_value::CellValue;
use rsheet_lib::{
    cells::{column_name_to_number, column_number_to_name},
    command_runner::CommandRunner,
};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use std::thread::spawn;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct Cell {
    value: CellValue,
    formula: String,
    dependencies: HashSet<String>,
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
        dependency: HashSet<String>,
        cell: String,
        lock: Arc<RwLock<Sheet>>,
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

        if check {
            relieve_dependencies(self.formula.clone(), cell.clone(), lock.clone());
        }

        self.timestamp = time;
        self.formula = formula;

        let mut check = false;

        if CellValue::Error("Could not cast Rhai return back to Cell Value.".to_string()) == value {
            self.value = value;
        } else {
            for i in dependency {
                let cell = cell.clone();

                if i == cell {
                    check = true;
                    continue;
                } else if self.dependencies.contains(&i) {
                    check = true;
                }
                let lock = lock.clone();

                spawn(move || {
                    add_dependencies(&i, cell, lock);
                });
            }

            self.value = if check {
                CellValue::Error(format!("Cell {cell} is self-referential",))
            } else {
                value
            };
        }

        if check {
            for j in self.dependencies.clone() {
                let cell = cell.clone();
                let lock = lock.clone();
                spawn(move || {
                    update_err_dependencies(j, lock, cell.clone());
                });
            }
        } else {
            for i in self.dependencies.clone() {
                let lock = lock.clone();
                spawn(move || {
                    update_dependencies(i, lock);
                });
            }
        }
    }

    pub fn update(&mut self, value: CellValue, lock: Arc<RwLock<Sheet>>) {
        self.value = value;

        for i in self.dependencies.clone() {
            let lock = lock.clone();
            spawn(move || {
                update_dependencies(i, lock);
            });
        }
    }

    pub fn update_err(&mut self, lock: Arc<RwLock<Sheet>>, target: String, cell: String) {
        if self.dependencies.contains(&cell) {
            self.value = CellValue::Error(format!("Cell {target} is self-referential",));
        } else {
            self.value = CellValue::Error("Reference a Error Cell.".to_string());
        }

        for i in self.dependencies.clone() {
            let cell = cell.clone();
            if i != cell {
                let lock = lock.clone();
                spawn(move || {
                    update_err_dependencies(i, lock, cell.clone());
                });
            }
        }
    }

    pub fn get_value(&self) -> CellValue {
        self.value.clone()
    }

    pub fn add_dependency(&mut self, cell: String) {
        self.dependencies.insert(cell);
    }

    pub fn remove_dependency(&mut self, cell: &str) {
        self.dependencies.retain(|x| x != cell);
    }

    pub fn get_formula(&self) -> &str {
        &self.formula
    }
}

pub fn add_dependencies(target: &str, cell: String, lock: Arc<RwLock<Sheet>>) {
    let cell_lock = get_or_insert(lock.clone(), target);
    let mut c = cell_lock.write().unwrap();
    c.add_dependency(cell.clone());
}

pub fn relieve_dependencies(formula: String, cell: String, lock: Arc<RwLock<Sheet>>) {
    spawn(move || {
        for i in CommandRunner::new(&formula).find_variables() {
            let regex =
                lazy_regex::regex_captures!(r"([A-Z]+)(\d+)(_([A-Z]+)(\d+))?", i.as_str()).unwrap();

            if regex.3.is_empty() {
                remove_dependencies(regex.1.to_string(), &cell, lock.clone());
            } else if regex.1 == regex.4 {
                for j in regex.2.parse::<u32>().unwrap()..=regex.5.parse::<u32>().unwrap() {
                    remove_dependencies(format!("{}{}", regex.1, j), &cell, lock.clone());
                }
            } else if regex.2 == regex.5 {
                for j in column_name_to_number(regex.1)..=column_name_to_number(regex.4) {
                    remove_dependencies(
                        format!("{}{}", column_number_to_name(j), regex.2),
                        &cell,
                        lock.clone(),
                    );
                }
            } else {
                let (row1, col1) = (
                    regex.2.parse::<u32>().unwrap(),
                    column_name_to_number(regex.1),
                );
                let (row2, col2) = (
                    regex.5.parse::<u32>().unwrap(),
                    column_name_to_number(regex.4),
                );
                for j in row1..=row2 {
                    for k in col1..=col2 {
                        remove_dependencies(
                            format!("{}{}", column_number_to_name(k), j),
                            &cell,
                            lock.clone(),
                        );
                    }
                }
            }
        }
    });
}

pub fn remove_dependencies(target: String, cell: &str, lock: Arc<RwLock<Sheet>>) {
    let cell_lock = get_or_insert(lock.clone(), &target);
    let mut c = cell_lock.write().unwrap();
    c.remove_dependency(cell);
}

pub fn update_err_dependencies(target_cell: String, lock: Arc<RwLock<Sheet>>, cell: String) {
    let cell_lock = get_or_insert(lock.clone(), &target_cell);
    let mut c = cell_lock.write().unwrap();
    c.update_err(lock.clone(), target_cell, cell)
}

pub fn update_dependencies(cell: String, lock: Arc<RwLock<Sheet>>) {
    let cell_lock = get_or_insert(lock.clone(), &cell);

    let binding = cell_lock.clone();
    let cell = binding.read().unwrap();
    let runner = CommandRunner::new(cell.get_formula());
    drop(cell);

    let (hash, _, check) = get_dependency_value(&runner, lock.clone());

    let value: CellValue = if check {
        runner.run(&hash)
    } else {
        CellValue::Error("Reference a Error Cell.".to_string())
    };

    let mut cell = cell_lock.write().unwrap();
    cell.update(value, lock);
}
