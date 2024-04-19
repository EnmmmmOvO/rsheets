use crate::sheet::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
pub struct Sheet {
    map: HashMap<String, Arc<RwLock<Cell>>>,
}

impl Sheet {
    pub fn new() -> Sheet {
        Sheet {
            map: HashMap::new(),
        }
    }

    pub fn get(&self, cell: &str) -> Option<Arc<RwLock<Cell>>> {
        self.map.get(cell).cloned()
    }

    pub fn insert(&mut self, cell: &str) -> Arc<RwLock<Cell>> {
        if let Some(lock) = self.map.get(cell) {
            return lock.clone();
        }

        let lock = Arc::new(RwLock::new(Cell::new_blank()));
        self.map.insert(cell.to_string(), lock.clone());
        lock
    }
}

pub fn get_or_insert(lock: Arc<RwLock<Sheet>>, cell: &str) -> Arc<RwLock<Cell>> {
    let guard = lock.read().unwrap();

    if let Some(lock) = guard.get(cell) {
        lock
    } else {
        drop(guard);
        let mut guard = lock.write().unwrap();
        guard.insert(cell)
    }
}
