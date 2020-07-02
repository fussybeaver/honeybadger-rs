//! Example that emits a standard error, and sends it to honeybadger

use honeybadger::{ConfigBuilder, Honeybadger};
use tokio::runtime::Runtime;

use std::fs::File;

async fn run() -> std::result::Result<(), honeybadger::errors::Error> {
    let api_token = "ffffff";
    let config = ConfigBuilder::new(api_token).build();
    let honeybadger = Honeybadger::new(config).unwrap();

    match make_error() {
        Ok(_) => Ok(()),
        Err(e) => {
            let boxed: Box<dyn std::error::Error> = e.into();
            Ok(honeybadger.notify(boxed, None).await?)
        }
    }
}

fn main() {
    let mut rt = Runtime::new().unwrap();
    rt.block_on(run()).unwrap();
}

fn make_error() -> Result<(), std::io::Error> {
    File::create("/permission_denied")?;
    Ok(())
}
