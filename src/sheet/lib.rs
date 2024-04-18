use crate::sheet::cell::Cell;
use crate::sheet::lock_pool::LockPool;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

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
        thread::sleep(std::time::Duration::from_secs(config.interval_time));
        lock_clone.lock().unwrap().motion();
        condvar_clone.notify_all();
    });

    (lock, condvar)
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

pub fn unlock(
    lock: Arc<Mutex<LockPool>>,
    condvar: Arc<Condvar>,
    row: u32,
    col: u32,
    cell_lock: MutexGuard<'_, Cell>,
) {
    let mut lock = lock.lock().unwrap();
    if lock.unlock(row, col, cell_lock) {
        condvar.notify_one();
    }
}
