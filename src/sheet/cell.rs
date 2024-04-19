use crate::sheet::lock_pool::LockPool;
use rsheet_lib::cell_value::CellValue;
use std::sync::{Arc, Condvar, Mutex};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct Cell {
    value: CellValue,
    formula: String,
    dependencies: Vec<(u32, u32)>,
    timestamp: SystemTime,
}

impl Cell {
    pub fn new_blank() -> Cell {
        Cell {
            value: CellValue::None,
            formula: String::new(),
            dependencies: Vec::new(),
            timestamp: SystemTime::UNIX_EPOCH,
        }
    }

    pub fn set_value(
        &mut self,
        formula: String,
        value: CellValue,
        time: SystemTime,
        lock: Arc<Mutex<LockPool>>,
        condvar: Arc<Condvar>,
    ) {
        if time <= self.timestamp {
            return;
        }

        self.timestamp = time;
        self.formula = formula;
        self.value = value;

        for (row, col) in &self.dependencies {
            println!("set_cell_value: {} {}", row, col);
        }
    }

    pub fn get_value(&self) -> CellValue {
        self.value.clone()
    }

    pub fn add_dependency(&mut self, row: u32, col: u32) {
        self.dependencies.push((row, col));
    }
}
