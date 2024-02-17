use proc_macro2::TokenStream;
use quote::quote;
use simple_error::Interpolate;
use syn::{
    parse_macro_input, spanned::Spanned, Data::Enum, DataEnum, DeriveInput, Error, Expr, ExprLit,
};

/**
This macro is used to derive the `Display` trait for an enum.
It requires the `#[error(...)]` attribute to be used on each variant of the enum.
The `#[error(...)]` attribute is used to specify the error message that will be
displayed when the variant is converted to a string.

```rust
use std::fmt::Display;

use simple_error_derive::SimpleError;

#[derive(Debug, SimpleError)]
    enum SomeError {
        #[error("hello unit")]
        Unit,
        #[error("hello {0:?} {1}")]
        Unnamed(UnnamedStructValue, i32),
        #[error("hello {message}")]
        Named { message: String },
    }

    #[derive(Debug)]
    struct UnnamedStructValue {
        value: i32,
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
```
*/
#[proc_macro_derive(SimpleError, attributes(error))]
pub fn thiserror(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    impl_display_error(&parse_macro_input!(input as DeriveInput))
        .map_err(|e| e.to_compile_error())
        .unwrap()
        .into()
}

fn impl_display_error(input: &DeriveInput) -> syn::Result<TokenStream> {
    let enum_name = &input.ident;
    let Enum(DataEnum { variants, .. }) = &input.data else {
        return Err(Error::new(input.span(), "This macro only supports enums"));
    };

    let match_arms = variants
        .iter()
        .map(|variant| {
            let attr = variant
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("error"))
                .ok_or(Error::new(
                    variant.span(),
                    "Missing #[error(...)] attribute",
                ))?;

            let Expr::Lit(ExprLit {
                lit: syn::Lit::Str(literal),
                ..
            }) = attr.parse_args::<Expr>()?
            else {
                return Err(Error::new(
                    attr.span(),
                    r#"String literal expected in #[error(...)] attribute e.g. #[error("error message")]"#,
                ));
            };

            let error_message = literal.value();
            let interpolator  = Interpolate::parse(&error_message, variant);
            Ok(quote!(#interpolator))

        })
        .collect::<Result<Vec<_>, _>>()?;

    let impls = quote! {
        impl std::fmt::Display for #enum_name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                match self {
                    #(#match_arms)*
                }
            }
        }
    };

    Ok(impls)
}
