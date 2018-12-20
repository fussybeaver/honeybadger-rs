//! Example that emits an error based on the chained_error crate, and sends it to honeybadger

extern crate futures;
extern crate honeybadger;
extern crate tokio;
#[macro_use]
extern crate error_chain;

error_chain! {
  errors {
    MyCustomError
  }
}

use futures::future;
use honeybadger::notice;
use honeybadger::{ConfigBuilder, Honeybadger};
use tokio::prelude::Future;
use tokio::runtime::Runtime;

fn main() {
    let api_token = "ffffff";
    let config = ConfigBuilder::new(api_token).build();
    let honeybadger = Honeybadger::new(config).unwrap();

    let mut rt = Runtime::new().unwrap();
    let future = future::result(make_error())
        .or_else(|e| honeybadger.notify(notice::Error::new(&e), None))
        .map_err(|e| println!("{:?}", e));

    rt.spawn(future);

    rt.shutdown_on_idle().wait().unwrap();
}

fn make_error() -> Result<()> {
    Err(ErrorKind::MyCustomError.into())
}