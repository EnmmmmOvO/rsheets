mod sheet;

use crate::sheet::create_lock_pool;
use lazy_regex::regex_captures;
use log::info;
use rsheet_lib::{
    cells::column_name_to_number,
    connect::{Manager, Reader, Writer},
    replies::Reply,
};
use std::error::Error;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub enum Action {
    Set(u32, u32, String),
    Get(u32, u32),
}

pub fn start_server<M>(mut manager: M) -> Result<(), Box<dyn Error>>
where
    M: Manager,
{
    let (mut recv, mut send) = manager.accept_new_connection().unwrap();
    let (lock, condvar) = create_lock_pool();

    loop {
        info!("Just got message");
        let msg = recv.read_message()?;

        match parse_input(&msg) {
            Ok(Action::Set(row, col, value)) => {}
            Ok(Action::Get(row, col)) => {}
            Err(e) => send.write_message(Reply::Error(e.to_string()))?,
        }
    }
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
                row.parse::<u32>().unwrap(),
                column_name_to_number(col),
            ))
        }
        Some((_, col, row, value)) => {
            if act != "set" {
                return Err("Unexpected Value Provided");
            }
            Ok(Action::Set(
                row.parse::<u32>().unwrap(),
                column_name_to_number(col),
                value.trim().to_string(),
            ))
        }
        _ => Err("Invalid Key Provided"),
    }
}
