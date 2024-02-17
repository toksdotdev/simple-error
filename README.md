## simple-error

Stripped down version of [`thiserror`](https://github.com/dtolnay/thiserror).

### Overview

This repo contains the following crates:
- [`simple-error`](./simple-error): Primitive crate for interpolating values into an interpolated string.
- [`simple-error-derive`](./simple-error-derive): Stripped down version of `thiserror::Error` with support for only interpolating enum values (never static values or with custom functions transformation).


### Usage

This crate provides a simple error type that can be used to create errors with a message and a source error.

```rust
#[derive(Debug, SimpleError)]
    enum SomeError {
        #[error("hello unit")]
        Unit,
        #[error("hello {0} {1}")]
        Unnamed(UnnamedStructValue, i32),
        #[error("hello {message}")]
        Named { message: String },
    }

    #[derive(Debug)]
    struct UnnamedStructValue {
        value: i32,
    }

    impl Display for UnnamedStructValue {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.value)
        }
    }

    #[test]
    fn test_display_error() {
        assert_eq!(SomeError::Unit.to_string(), "hello unit");
        assert_eq!(
            SomeError::Named {
                message: "world".to_string(),
            }
            .to_string(),
            "hello world"
        );
        assert_eq!(
            SomeError::Unnamed(UnnamedStructValue { value: 42 }, 45).to_string(),
            "hello 42 45"
        );
    }
}
```
