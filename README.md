[![Build Status](https://circleci.com/gh/fussybeaver/honeybadger-rs/tree/master.svg?style=svg)](https://circleci.com/gh/fussybeaver/honeybadger-rs/cargo-readme/tree/master)

# honeybadger

An unofficial Honeybadger Rust client

## Description

[Honeybadger][1] is a service that receives, stores and alerts on
application errors and outages.  This library is a community-provided client for the [Honeybadger Exceptions API](https://docs.honeybadger.io/api/exceptions.html).

Underneath, the client uses a [Tokio](https://tokio.rs/)-based version of
[Hyper](https://hyper.rs/), and leverages
[ErrorChain](https://docs.rs/error-chain/0.12.0/error_chain/) to support backtraces. Basic
support for the [Failure](https://github.com/rust-lang-nursery/failure) Error struct exists
through a `From` trait, and hence the possibility for further compatibility with other custom
Error implementations.

## Example

Assuming the project is setup to use
[ErrorChain](http://brson.github.io/2016/11/30/starting-with-error-chain), the following
example will execute code in `do_work`, send a honeybadger exception if it fails, and
subsequently end the program.

```rust
use tokio::prelude::*;
use tokio::prelude::future::result;
use tokio::runtime::run;

fn do_work() -> Result<()> {

  // write code ...

  Ok(())
}

// let api_token = "...";
let config = ConfigBuilder::new(api_token).build();
let mut hb = Honeybadger::new(config).unwrap();

let work = result(do_work())
  .or_else(move |e| result(hb.create_payload(&e, None))
                      .and_then(move |payload| hb.notify(payload)))
  .map_err(|e| println!("error = {:?}", e));

run(work);
```
[1]: https://www.honeybadger.io/

License: MIT
