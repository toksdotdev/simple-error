## simple-error

Stripped down version of [`thiserror`](https://github.com/dtolnay/thiserror).

### Overview

This repo contains the following crates:
- [`simple-error`](./simple-error): Primitive crate for interpolating values into an interpolated string.
- [`simple-error-derive`](./simple-error-derive): Stripped down version of `thiserror::Error` with support for only interpolating enum values (never static values or with custom functions transformation).


### Usage

```rust
use std::fmt::Display;

use simple_error_derive::SimpleError;

#[derive(Debug, SimpleError)]
enum SomeError<'a> {
    #[error("Unit error")]
    Unit,

    #[error("Unnamed error: {0:?}, {1}, 0x{2:0x}")]
    Unnamed(State, &'a str, i32),

    #[error("Named error: {message}")]
    Named { message: String },
}

#[derive(Debug)]
struct State {
    code: i32,
}

fn unnamed_error() -> Result<(), SomeError<'static>> {
    Err(SomeError::Unnamed(
        State { code: 2 },
        "state error",
        32
    ))
}

fn named_error() -> Result<(), SomeError<'static>> {
    Err(SomeError::Named {
        message: "critical error".to_string(),
    })
}


assert_eq!(SomeError::Unit.to_string(), "Unit error");
assert_eq!(
    unnamed_error().unwrap_err().to_string(),
    "Unnamed error: State { code: 2 }, state error, 0x20"
);
assert_eq!(
    named_error().unwrap_err().to_string(),
    "Named error: critical error"
);
```
