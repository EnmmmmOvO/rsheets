use crate::sheet::lock_pool::{get_or_insert, unlock, LockPool};
use crate::unlock;
use rsheet_lib::cell_value::CellValue;
use rsheet_lib::cells::{column_name_to_number, column_number_to_name};
use rsheet_lib::command_runner::{CellArgument, CommandRunner};
use rsheet_lib::connect::ConnectionError;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::thread::sleep;
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

pub fn create_lock_pool() -> (Arc<Mutex<LockPool>>, Arc<Condvar>) {
    let mut data = String::new();
    File::open("config.json")
        .expect("file should open read only")
        .read_to_string(&mut data)
        .expect("error reading the file");

    let config: Config = serde_json::from_str(&data).expect("error while reading json");

    let lock = Arc::new(Mutex::new(LockPool::new(&config)));
    let condvar = Arc::new(Condvar::new());

    let lock_clone = lock.clone();
    let condvar_clone = condvar.clone();
    thread::spawn(move || loop {
        sleep(std::time::Duration::from_secs(config.interval_time));
        lock_clone.lock().unwrap().motion();
        condvar_clone.notify_all();
    });

    (lock, condvar)
}

pub fn set_cell_value(
    row: u32,
    col: u32,
    formula: String,
    lock: Arc<Mutex<LockPool>>,
    condvar: Arc<Condvar>,
) -> Result<(), ConnectionError> {
    let time = SystemTime::now();
    let runner = CommandRunner::new(&formula);
    let (hash, mut dependency) = get_dependency_value(&runner, lock.clone(), condvar.clone());

    let value = if dependency.contains(&(row, col)) {
        dependency.clear();
        CellValue::Error(format!("Cell {}{} is self-referential.", column_number_to_name(col), row + 1))
    } else {
        runner.run(&hash)
    };

    let cell_lock = get_or_insert(lock.clone(), condvar.clone(), row, col);
    let mut cell = cell_lock.lock().unwrap();
    cell.set_value(
        formula.clone(),
        value,
        time,
        dependency,
        (row, col),
        (lock.clone(), condvar.clone()),
    );

    unlock!(lock, condvar, row, col, cell_lock, cell);

    Ok(())
}

pub fn get_dependency_value(
    runner: &CommandRunner,
    lock: Arc<Mutex<LockPool>>,
    condvar: Arc<Condvar>,
) -> (HashMap<String, CellArgument>, HashSet<(u32, u32)>) {
    let mut hash = HashMap::new();
    let mut record = HashSet::new();

    for i in runner.find_variables() {
        let regex =
            lazy_regex::regex_captures!(r"([A-Z]+)(\d+)(_([A-Z]+)(\d+))?", i.as_str()).unwrap();

        hash.insert(
            i.clone(),
            if regex.3.is_empty() {
                let (target_row, target_col) = (
                    regex.2.parse::<u32>().unwrap() - 1,
                    column_name_to_number(regex.1),
                );
                record.insert((target_row, target_col));
                CellArgument::Value(get_cell_value(
                    target_row,
                    target_col,
                    lock.clone(),
                    condvar.clone(),
                ))
            } else if regex.1 == regex.4 {
                let target_col = column_name_to_number(regex.1);
                let mut vec = vec![];
                for i in regex.2.parse::<u32>().unwrap() - 1..=regex.5.parse::<u32>().unwrap() - 1 {
                    record.insert((i, target_col));
                    vec.push(get_cell_value(i, target_col, lock.clone(), condvar.clone()));
                }
                CellArgument::Vector(vec)
            } else if regex.2 == regex.5 {
                let target_row = regex.2.parse::<u32>().unwrap() - 1;
                let mut vec = vec![];
                for i in column_name_to_number(regex.1)..=column_name_to_number(regex.4) {
                    record.insert((target_row, i));
                    vec.push(get_cell_value(target_row, i, lock.clone(), condvar.clone()));
                }
                CellArgument::Vector(vec)
            } else {
                let (row1, col1) = (
                    regex.2.parse::<u32>().unwrap() - 1,
                    column_name_to_number(regex.1),
                );
                let (row2, col2) = (
                    regex.5.parse::<u32>().unwrap() - 1,
                    column_name_to_number(regex.4),
                );
                let mut vec = vec![];

                for i in row1..=row2 {
                    let mut temp = vec![];
                    for j in col1..=col2 {
                        record.insert((i, j));
                        temp.push(get_cell_value(i, j, lock.clone(), condvar.clone()));
                    }
                    vec.push(temp);
                }

                CellArgument::Matrix(vec)
            },
        );
    }

    (hash, record)
}

pub fn get_cell_value(
    row: u32,
    col: u32,
    lock: Arc<Mutex<LockPool>>,
    condvar: Arc<Condvar>,
) -> CellValue {
    let cell_lock = get_or_insert(lock.clone(), condvar.clone(), row, col);

    let cell = cell_lock.lock().unwrap();
    let value = cell.get_value();

    unlock!(lock, condvar, row, col, cell_lock, cell);

    value
}
