//! Example that emits an error based on the chained_error crate, and sends it to honeybadger

#[macro_use]
extern crate failure;

#[derive(Fail, Debug)]
#[fail(display = "Failure error")]
struct MyCustomError;

use honeybadger::{ConfigBuilder, Honeybadger};
use tokio::runtime::Runtime;

async fn run() -> std::result::Result<(), honeybadger::errors::Error> {
    let api_token = "ffffff";
    let config = ConfigBuilder::new(api_token).build();
    let honeybadger = Honeybadger::new(config).unwrap();

    match make_error() {
        Ok(_) => Ok(()),
        Err(e) => Ok(honeybadger.notify(&e, None).await?)
    }
}

fn main() {
    let mut rt = Runtime::new().unwrap();
    rt.block_on(run()).unwrap();
}

fn make_error() -> Result<(), failure::Error> {
    Err(MyCustomError {}.into())
}
