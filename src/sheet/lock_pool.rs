use crate::sheet::cell::Cell;
use crate::sheet::lib::Config;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex};

#[macro_export]
macro_rules! unlock {
    ($lock:expr, $condvar:expr, $row:expr, $col:expr, $mutex:expr, $guard:expr) => {
        drop($guard);
        drop($mutex);
        unlock($lock.clone(), $condvar.clone(), $row, $col);
    };
}

#[derive(Debug)]
pub struct LockPool {
    used: u32,
    capacity: u32,
    min: u32,
    max: u32,
    wait: u32,
    visit: u32,
    expansion_threshold: f32,
    expansion_multiplier: f32,
    contraction_threshold: f32,
    contraction_multiplier: f32,

    free_list: VecDeque<(u32, u32)>,
    sheet: Vec<Vec<Cell>>,
    map: HashMap<(u32, u32), Arc<Mutex<Cell>>>,
}

impl LockPool {
    pub fn new(config: &Config) -> LockPool {
        LockPool {
            used: 0,
            capacity: config.min,
            min: config.min,
            max: config.max,
            wait: 0,
            visit: 0,
            expansion_threshold: config.expansion_threshold,
            expansion_multiplier: config.expansion_multiplier,
            contraction_threshold: config.contraction_threshold,
            contraction_multiplier: config.contraction_multiplier,
            free_list: VecDeque::new(),
            sheet: vec![vec![]],
            map: HashMap::new(),
        }
    }

    pub fn get_or_insert(&mut self, row: u32, col: u32) -> Option<Arc<Mutex<Cell>>> {
        self.expansion(row, col);
        if let Some(lock) = self.map.get(&(row, col)) {
            if Arc::strong_count(lock) == 1 {
                for (i, (r, c)) in self.free_list.iter().enumerate() {
                    if *r == row && *c == col {
                        self.free_list.remove(i);
                        break;
                    }
                }
            }
            self.visit += 1;
            return Some(lock.clone());
        } else if self.used == self.capacity {
            if self.free_list.is_empty() {
                self.wait += 1;
                return None;
            }

            let (row, col) = self.free_list.pop_front().unwrap();
            self.rewrite(row, col);
        } else {
            self.used += 1;
        }

        self.visit += 1;
        let new_lock = Arc::new(Mutex::new(self.sheet[row as usize][col as usize].clone()));
        self.map.insert((row, col), new_lock.clone());

        Some(new_lock)
    }

    pub fn rewrite(&mut self, row: u32, col: u32) {
        let cell = self.map.remove_entry(&(row, col)).unwrap();
        let cell = cell.1.lock().unwrap();
        self.sheet[row as usize][col as usize] = cell.clone();
    }

    pub fn unlock(&mut self, row: u32, col: u32) -> bool {
        if Arc::strong_count(self.map.get(&(row, col)).unwrap()) == 1 {
            self.free_list.push_back((row, col));
            return true;
        }
        false
    }

    pub fn motion(&mut self) {
        if self.wait as f32 / self.capacity as f32 > self.expansion_threshold {
            let temp = (self.capacity as f32 * self.expansion_multiplier) as u32;
            self.capacity = if temp < self.max { temp } else { self.max };
        } else if self.wait == 0
            && self.visit as f32 / (self.capacity as f32) < self.contraction_threshold
            && self.free_list.len() as u32 + self.capacity - self.used
                > ((self.capacity as f32) * (1.0 - self.contraction_multiplier) * 2.0) as u32
        {
            let target = (self.capacity as f32 * self.contraction_multiplier) as u32;

            if target >= self.min {
                if self.used > target {
                    let delete = self.used - target;
                    for _ in 0..delete {
                        let (row, col) = self.free_list.pop_front().unwrap();
                        self.rewrite(row, col);
                        self.used -= 1;
                    }
                }
                self.capacity = target;
            }
        }
        self.wait = 0;
        self.visit = 0;
    }

    fn expansion(&mut self, row: u32, col: u32) {
        if row as usize >= self.sheet.len() {
            for _ in 0..(row as usize - self.sheet.len() + 1) {
                self.sheet.push(vec![]);
            }
        }

        if col as usize >= self.sheet[row as usize].len() {
            for _ in 0..(col - self.sheet[row as usize].len() as u32 + 1) {
                self.sheet[row as usize].push(Cell::new_blank());
            }
        }
    }

    // pub fn print(&self) {
    //     println!("map: {:?}", self.map,);
    // }
}

pub fn get_or_insert(
    lock: Arc<Mutex<LockPool>>,
    condvar: Arc<Condvar>,
    row: u32,
    col: u32,
) -> Arc<Mutex<Cell>> {
    let mut guard = lock.lock().unwrap();
    loop {
        if let Some(p) = guard.get_or_insert(row, col) {
            return p;
        }
        guard = condvar.wait(guard).unwrap();
    }
}

pub fn unlock(lock: Arc<Mutex<LockPool>>, condvar: Arc<Condvar>, row: u32, col: u32) {
    let mut lock = lock.lock().unwrap();
    if lock.unlock(row, col) {
        condvar.notify_one();
    }
}
