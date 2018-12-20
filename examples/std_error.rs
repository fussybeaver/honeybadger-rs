//! Example that emits a standard error, and sends it to honeybadger

extern crate futures;
extern crate honeybadger;
extern crate tokio;

use futures::future;
use honeybadger::{ConfigBuilder, Honeybadger};
use tokio::prelude::Future;
use tokio::runtime::Runtime;

use std::fs::File;

fn main() {
    let api_token = "ffffff";
    let config = ConfigBuilder::new(api_token).build();
    let honeybadger = Honeybadger::new(config).unwrap();

    let mut rt = Runtime::new().unwrap();
    let future = future::result(make_error())
        .or_else(|e| {
            let boxed: Box<std::error::Error> = e.into();
            honeybadger.notify(boxed, None)
        })
        .map_err(|e| println!("{:?}", e));

    rt.spawn(future);

    rt.shutdown_on_idle().wait().unwrap();
}

fn make_error() -> Result<(), std::io::Error> {
    File::create("/permission_denied")?;
    Ok(())
}
