// Increase the compiler's recursion limit for the `error_chain` crate.
#![recursion_limit = "1024"]

extern crate backtrace;
#[macro_use]
extern crate error_chain;
extern crate futures;
extern crate hostname;
extern crate http;
extern crate hyper;
extern crate hyper_tls;
#[macro_use] extern crate log;
extern crate os_type;
extern crate native_tls;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde;
extern crate tokio;

mod honeybadger;
pub mod errors;
pub mod notice;

// export 
pub use honeybadger::{Honeybadger, ConfigBuilder};
