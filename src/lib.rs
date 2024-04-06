use rsheet_lib::connect::{Manager, Reader, Writer};
use rsheet_lib::replies::Reply;

use std::error::Error;

use log::info;

pub fn start_server<M>(mut manager: M) -> Result<(), Box<dyn Error>>
where
    M: Manager,
{
    let (mut recv, mut send) = manager.accept_new_connection().unwrap();
    loop {
        info!("Just got message");
        let msg = recv.read_message()?;
        send.write_message(Reply::Error(format!("{msg:?}")))?;
    }
}
