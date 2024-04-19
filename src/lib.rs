mod sheet;

use crate::sheet::{get_cell_value, set_cell_value, Sheet};
use lazy_regex::regex_captures;
use log::info;
use rsheet_lib::cell_value::CellValue;
use rsheet_lib::connect::{ConnectionError, ReaderWriter};
use rsheet_lib::{
    connect::{Manager, Reader, Writer},
    replies::Reply,
};
use std::sync::{Arc, RwLock};
use std::thread::spawn;

#[derive(Debug)]
pub enum Action {
    Set(String, String),
    Get(String),
}

fn create_new_thread<M>(
    mut recv: <<M as Manager>::ReaderWriter as ReaderWriter>::Reader,
    mut send: <<M as Manager>::ReaderWriter as ReaderWriter>::Writer,
    lock: Arc<RwLock<Sheet>>,
) -> Result<(), ConnectionError>
where
    M: Manager + Send + 'static,
{
    while let Ok(msg) = recv.read_message() {
        info!("Just got message");
        let lock = lock.clone();

        match parse_input(&msg) {
            Ok(Action::Set(cell, value)) => {
                set_cell_value(cell, value, lock)?;
            }
            Ok(Action::Get(cell)) => match get_cell_value(cell.clone(), lock) {
                CellValue::Error(e) => {
                    if e == "Reference a Error Cell." {
                        send.write_message(Reply::Error("Reference a Error Cell.".to_string()))?;
                    } else {
                        send.write_message(Reply::Value(cell, CellValue::Error(e)))?;
                    }
                }
                value => {
                    send.write_message(Reply::Value(cell, value))?;
                }
            },
            Err(e) => {
                send.write_message(Reply::Error(e.to_string()))?;
            }
        }
    }
    Ok(())
}

pub fn start_server<M>(mut manager: M) -> Result<(), ConnectionError>
where
    M: Manager + Send + 'static,
{
    let lock = Arc::new(RwLock::new(Sheet::new()));
    let mut record = vec![];
    while let Ok((recv, send)) = manager.accept_new_connection() {
        let lock = lock.clone();
        record.push(spawn(move || -> Result<(), ConnectionError> {
            create_new_thread::<M>(recv, send, lock)
        }));
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

    match regex_captures!("^([A-Z]+[1-9][0-9]*)(.*)$", args.trim()) {
        Some((.., cell, "")) => {
            if act != "get" {
                return Err("Losing Required Value");
            }
            Ok(Action::Get(cell.to_string()))
        }
        Some((_, cell, value)) => {
            if act != "set" {
                return Err("Unexpected Value Provided");
            }
            Ok(Action::Set(cell.to_string(), value.trim().to_string()))
        }
        _ => Err("Invalid Key Provided"),
    }
}
