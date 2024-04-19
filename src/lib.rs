mod sheet;

use std::sync::Mutex;
use std::sync::Arc;
use crate::sheet::{create_lock_pool, get_cell_value, LockPool, set_cell_value};
use lazy_regex::regex_captures;
use log::info;
use rsheet_lib::cell_value::CellValue;
use rsheet_lib::connect::{ConnectionError, ReaderWriter};
use rsheet_lib::{
    cells::column_name_to_number,
    connect::{Manager, Reader, Writer},
    replies::Reply,
};
use std::sync::Condvar;
use std::thread::spawn;

#[derive(Debug)]
pub enum Action {
    Set(u32, u32, String),
    Get(u32, u32, String),
}

fn create_new_thread<M>(mut recv: <<M as Manager>::ReaderWriter as ReaderWriter>::Reader, mut send: <<M as Manager>::ReaderWriter as ReaderWriter>::Writer, lock: Arc<Mutex<LockPool>>, condvar: Arc<Condvar>) -> Result<(), ConnectionError>
where
    M: Manager + Send + 'static
{
    loop {
        let msg;
        if let Ok(m) = recv.read_message() {
            msg = m;
        } else {
            break;
        }
        info!("Just got message");
        let lock = lock.clone();
        let condvar = condvar.clone();

        match parse_input(&msg) {
            Ok(Action::Set(row, col, value)) => {
                set_cell_value(row, col, value, lock, condvar)?;
            }
            Ok(Action::Get(row, col, cell)) => {
                send.write_message(Reply::Value(cell, get_cell_value(row, col, lock, condvar)))?;
            }
            Err(e) => {
                send.write_message(Reply::Error(e.to_string()))?;
            }
        }
    }
    Ok(())
}

pub fn start_server<M>(mut manager: M) -> Result<(), ConnectionError>
where
    M: Manager + Send + 'static
{
    let (lock, condvar) = create_lock_pool();
    let mut record = vec![];
    loop {
        if let Ok((recv, send)) = manager.accept_new_connection() {
            let lock = lock.clone();
            let condvar = condvar.clone();
            record.push(spawn(move || -> Result<(), ConnectionError> {
                create_new_thread::<M>(recv, send, lock, condvar)
            }));
        } else {
            break;
        }
    }
    for handle in record {
        handle.join().unwrap()?;
    }
    Ok(())
}

fn parse_input(input: &str) -> Result<Action, &str> {
    let input = input.trim();

    if input == "set" || input == "get" {
        return Err("Losing Required Value");
    }

    let (act, args) = match regex_captures!("^(set|get) (.*)", input) {
        Some((.., "")) => Err("Losing Required Value"),
        Some((_, act, args)) => Ok((act, args)),
        _ => Err("Invalid Command Provided"),
    }?;

    match regex_captures!("^([A-Z]+)([1-9][0-9]*)(.*)$", args.trim()) {
        Some((.., col, row, "")) => {
            if act != "get" {
                return Err("Losing Required Value");
            }
            Ok(Action::Get(
                row.parse::<u32>().unwrap() - 1,
                column_name_to_number(col),
                format!("{}{}", col, row),
            ))
        }
        Some((_, col, row, value)) => {
            if act != "set" {
                return Err("Unexpected Value Provided");
            }
            Ok(Action::Set(
                row.parse::<u32>().unwrap() - 1,
                column_name_to_number(col),
                value.trim().to_string(),
            ))
        }
        _ => Err("Invalid Key Provided"),
    }
}
