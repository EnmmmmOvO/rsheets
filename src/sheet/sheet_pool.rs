use crate::sheet::cell::Cell;
use petgraph::Graph;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread::spawn;

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

    pub fn insert(
        &mut self,
        cell: &str,
        new: Arc<RwLock<Cell>>,
        graph: Arc<RwLock<Graph<Arc<RwLock<Cell>>, ()>>>,
    ) -> Arc<RwLock<Cell>> {
        if let Some(lock) = self.map.get(cell) {
            spawn(move || {
                let node = new.read().unwrap().get_value_and_node().1;
                let mut graph = graph.write().unwrap();
                graph.remove_node(node);
            });
            return lock.clone();
        }

        self.map.insert(cell.to_string(), new.clone());
        new
    }
}

pub fn get_or_insert(
    lock: Arc<RwLock<Sheet>>,
    cell: &str,
    graph: Arc<RwLock<Graph<Arc<RwLock<Cell>>, ()>>>,
) -> Arc<RwLock<Cell>> {
    let guard = lock.read().unwrap();

    if let Some(sheet) = guard.get(cell) {
        sheet
    } else {
        drop(guard);

        let new = Arc::new(RwLock::new(Cell::new_blank(cell.to_string())));
        let g_temp = graph.clone();
        let mut g = g_temp.write().unwrap();
        let node = g.add_node(new.clone());
        new.write().unwrap().set_node(node);
        drop(g);

        let mut guard = lock.write().unwrap();
        guard.insert(cell, new, graph)
    }
}
