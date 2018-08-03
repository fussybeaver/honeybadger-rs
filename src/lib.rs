//! An unofficial Honeybadger Rust client
//!
//! # Description
//!
//! [Honeybadger][1] is a service that receives, stores and alerts on
//! application errors and outages.  This library is a community-provided client for the [Honeybadger Exceptions API](https://docs.honeybadger.io/api/exceptions.html).
//! 
//! Underneath, the client uses a [Tokio](https://tokio.rs/)-based version of
//! [Hyper](https://hyper.rs/), and leverages
//! [ErrorChain](https://docs.rs/error-chain/0.12.0/error_chain/) to support backtraces. Basic
//! support for the [Failure](https://github.com/rust-lang-nursery/failure) Error struct exists
//! through a `From` trait, and hence the possibility for further compatibility with other custom
//! Error implementations.
//!
//! # Example
//!
//! Assuming the project is setup to use
//! [ErrorChain](http://brson.github.io/2016/11/30/starting-with-error-chain), the following
//! example will execute code in `do_work`, send a honeybadger exception if it fails, and
//! subsequently end the program.  
//!
//! ```rust
//! # #[macro_use] extern crate error_chain;
//! # extern crate honeybadger;
//! # extern crate tokio;
//! # error_chain! {
//! # }
//! use tokio::prelude::*;
//! use tokio::prelude::future::result;
//! use tokio::runtime::run;
//!
//! fn do_work() -> Result<()> {
//!
//!   // write code ...
//!
//!   Ok(())
//! }
//!
//! # fn main() {
//! # use honeybadger::{ConfigBuilder, Honeybadger};
//! # let api_token = "ffffff";
//! // let api_token = "...";
//! let config = ConfigBuilder::new(api_token).build();
//! let mut hb = Honeybadger::new(config).unwrap();
//!
//! let work = result(do_work())
//!   .or_else(move |e| result(hb.create_payload(&e, None))
//!                       .and_then(move |payload| hb.notify(payload)))
//!   .map_err(|e| println!("error = {:?}", e)); 
//!
//! run(work);
//! # }
//! ```
//![1]: https://www.honeybadger.io/
//
// Increase the compiler's recursion limit for the `error_chain` crate.
#![recursion_limit = "1024"]

extern crate backtrace;
#[macro_use]
extern crate error_chain;
extern crate failure;
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
#[cfg(test)]
extern crate yup_hyper_mock as hyper_mock;

mod honeybadger;
pub mod errors;
pub mod notice;

// export 
pub use honeybadger::{Honeybadger, ConfigBuilder};
