//! An unofficial Honeybadger Rust client
//!
//! # Description
//!
//! [Honeybadger][1] is a service that receives, stores and alerts on
//! application errors and outages.  This library is a community-provided client for the [Honeybadger Exceptions API](https://docs.honeybadger.io/api/exceptions.html).
//!
//! Underneath, the client uses a [Tokio](https://tokio.rs/)-based version of
//! [Hyper](https://hyper.rs/). Familiarity with Tokio-based systems is recommended.
//!
//! # Error library compatibility
//!
//! The library provides convenience conversion traits and methods to generate a Honeybadger payload for use in the [`Honeybadger::notify`](https://docs.rs/honeybadger/0.1.3/honeybadger/struct.Honeybadger.html#method.notify) API endpoint, based on popular error Rust libraries.
//!
//!  - a [From](https://doc.rust-lang.org/std/convert/trait.From.html) conversion trait enables use of a `failure::Error`, if using the
//! [failure](https://rust-lang-nursery.github.io/failure/) crate.
//!
//!  - the
//!  [`notice::Error::new`](./notice/struct.Error.html#method.new) convenience method creates a `notice::Error` Honeybadger
//!  payload, if using the [error_chain](https://docs.rs/error-chain/0.12.0/error_chain/) crate.
//!
//!  - alternatively, a [From](https://doc.rust-lang.org/std/convert/trait.From.html) trait allows use of a simple `Box<std::error::Error>`, if using errors from the Rust standard library.
//!
//! Backtraces are only supported in the ErrorChain and Failure crates.
//!
//! # Example
//!
//! Assuming the project is setup to use
//! [ErrorChain](http://brson.github.io/2016/11/30/starting-with-error-chain), the following
//! example will execute code in `do_work`, send a honeybadger exception if it fails, and
//! subsequently end the program.  
//!
//! ```rust, no_run
//! # #[macro_use] extern crate error_chain;
//! # error_chain! {
//! # }
//! use tokio::runtime::Runtime;
//!
//! async fn do_work() -> Result<()> {
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
//! let mut rt = Runtime::new().unwrap();
//!
//! let future = async move {
//!   match do_work().await {
//!     Ok(_) => Ok(()),
//!     Err(e) => hb.notify(honeybadger::notice::Error::new(&e), None).await
//!   }
//! };
//!
//! rt.block_on(future).unwrap();
//! # }
//! ```
//![1]: https://www.honeybadger.io/
//!
//! Please check the examples folder for further alternatives.
//!
//
// Increase the compiler's recursion limit for the `error_chain` crate.
#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

pub mod errors;
mod honeybadger;
pub mod notice;

// export
pub use crate::honeybadger::{ConfigBuilder, Honeybadger};
