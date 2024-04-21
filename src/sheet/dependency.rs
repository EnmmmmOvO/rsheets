use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub struct Dependency {
    err: HashSet<String>,
    check: bool,
    normal: HashSet<String>,
}

pub enum Status {
    ErrChanged,
    Err,
    True,
}

impl Dependency {
    pub fn new(cell: String) -> Arc<Mutex<Dependency>> {
        let mut normal = HashSet::new();
        normal.insert(cell);
        Arc::new(Mutex::new(Dependency {
            err: HashSet::new(),
            check: true,
            normal,
        }))
    }

    pub fn new_err() -> Arc<Mutex<Dependency>> {
        Arc::new(Mutex::new(Dependency {
            err: HashSet::new(),
            check: false,
            normal: HashSet::new(),
        }))
    }

    fn check_normal(&self, cell: &str) -> bool {
        self.normal.contains(cell)
    }

    fn check_err(&self, cell: &str) -> bool {
        self.err.contains(cell)
    }

    fn check(&self) -> bool {
        self.check
    }

    fn change_check(&mut self) {
        self.check = false;
    }

    fn insert_err(&mut self, cell: String) {
        self.err.insert(cell);
    }

    fn insert_normal(&mut self, cell: String) {
        self.normal.insert(cell);
    }
}

pub fn check_or_insert_normal(dependency: Arc<Mutex<Dependency>>, cell: String) -> Status {
    let mut guard = dependency.lock().unwrap();
    if !guard.check() {
        if guard.check_err(&cell) {
            return Status::ErrChanged;
        } else {
            guard.insert_err(cell);
            return Status::Err;
        }
    }

    if guard.check_normal(&cell) {
        guard.change_check();
        guard.insert_err(cell);
        Status::Err
    } else {
        guard.insert_normal(cell);
        Status::True
    }
}
