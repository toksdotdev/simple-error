use std::collections::BTreeSet;

#[cfg(feature = "display")]
use proc_macro2::Ident;

#[cfg(feature = "display")]
use quote::quote;

use syn::Variant;

/// The struct that holds the interpolated format string and
/// the fields used in the format string.
///
/// The default implementation of `ToTokens` is used to generate
/// the match arms for the `Display` trait implementation.
///
/// You can also use the fields exposed on the struct to generate
/// your own match arms for any other trait implementation.
pub struct Interpolate<'a> {
    /// The variant for which the format string is being interpolated.
    pub variant: &'a Variant,

    /// The format string with the interpolated fields:
    /// - For named values, `{name}`, it remains as untouched e.g. `{name}`.
    /// - For positional values, `{[0-9]*}`, it is replaced with `__0`, `__1`, etc, where
    ///   the number is the index of the interpolated value. If the index is manually
    ///   specified, it is used instead of an auto-incremented index.
    pub rewritten_text: String,

    /// Identifiers used in the interpolated text.
    pub identifiers: BTreeSet<String>,
}

impl Interpolate<'_> {
    /// Parse the format text and extract the fields to be interpolated.
    /// Returns a tuple of the fields and the format string with the interpolated
    /// fields replaced with the __ prefix (and for positional values, __0, __1, etc.)
    pub fn parse<'a>(fmt_text: impl AsRef<str>, variant: &'a Variant) -> Interpolate<'a> {
        let (rewritten_text, identifiers) = parse_internal(fmt_text);

        Interpolate {
            variant,
            rewritten_text,
            identifiers,
        }
    }
}

/// Parse the text and extract the identifiers to be interpolated.
fn parse_internal(text: impl AsRef<str>) -> (String, BTreeSet<String>) {
    let mut chars = text.as_ref().chars().peekable();
    let (mut identifers, mut text, mut positional_index) = (BTreeSet::new(), String::new(), -1);

    while let Some(c) = chars.next() {
        if c != '{' {
            text.push(c);
            continue;
        }

        // If the next character is also a '{', then it's an escaped '{'
        if let Some('{') = chars.peek() {
            text.push_str("{{");
            chars.next();
            continue;
        }

        let (mut identifier, mut traits) = ("".to_string(), None);
        while let Some(c) = chars.next() {
            if c == ':' {
                // Collect everything after the ':' as the trait name until we find the closing '}'.
                while let Some(c) = chars.peek() {
                    if *c == '}' {
                        break;
                    }

                    traits.get_or_insert("".to_string()).push(*c);
                    chars.next();
                }

                continue;
            }

            if c == '}' {
                // If no field name was parsed bfore the ':', then it's a positional value;
                // so we need to add the index to the field name
                if identifier.is_empty() {
                    positional_index += 1;
                    identifier.push_str(&format!("__{}", positional_index));
                }

                if identifier.parse::<u8>().is_ok() {
                    identifier = format!("__{}", identifier);
                }

                let traits = traits.as_ref().map(|c| format!(":{c}")).unwrap_or_default();
                text.push_str(&format!("{{{}{}}}", &identifier, traits));
                identifers.insert(identifier.clone());
                break;
            }

            identifier.push(c);
        }
    }

    (text, identifers)
}

#[cfg(feature = "display")]
impl quote::ToTokens for Interpolate<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let variant_name = &self.variant.ident;
        let interpolated_text = &self.rewritten_text;

        let mappings = match &self.variant.fields {
            syn::Fields::Unit => {
                quote! {
                    Self::#variant_name => write!(f, #interpolated_text),
                }
            }
            syn::Fields::Unnamed(fields) => {
                let fields = fields.unnamed.iter().collect::<Vec<_>>();
                let assignments = fields.iter().flat_map(|field| {
                    field
                        .ident
                        .as_ref()
                        .and_then(|ident| build_ident_assignment(ident, &self.identifiers))
                });

                let fields_ident = self
                    .identifiers
                    .iter()
                    .map(|ident| Ident::new(ident, proc_macro2::Span::call_site()));

                quote! {
                    Self::#variant_name(#(#fields_ident,)* ..) => write!(f, #interpolated_text, #(#assignments),*),
                }
            }
            syn::Fields::Named(fields) => {
                let fields = fields.named.iter().collect::<Vec<_>>();
                let fields_ident = fields.iter().flat_map(|field| &field.ident);

                quote! {
                    Self::#variant_name { #(#fields_ident,)* } => write!(f, #interpolated_text),
                }
            }
        };

        tokens.extend(mappings);
    }
}

#[cfg(feature = "display")]
/// Build the assignment for the field if it is used in the format string.
fn build_ident_assignment(
    ident: &Ident,
    used_fields: &BTreeSet<String>,
) -> Option<proc_macro2::TokenStream> {
    use quote::format_ident;

    // If the field is not present in the format string, then we don't need to interpolate it
    if !used_fields.contains(&ident.to_string()) {
        return None;
    }

    let ident = format_ident!("__{}", ident);
    Some(quote! { #ident = self.#ident })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::parse_internal;

    fn to_set<T: ToString>(values: &[T]) -> BTreeSet<String> {
        values.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn test_parse_fmt_string() {
        assert_eq!(
            parse_internal("Hello, {name}!"),
            ("Hello, {name}!".to_string(), to_set(&["name"]))
        );

        assert_eq!(
            parse_internal("Hello, {name}! {age}"),
            ("Hello, {name}! {age}".to_string(), to_set(&["name", "age"]),)
        );

        assert_eq!(
            parse_internal("Hello, {0}! {1}"),
            ("Hello, {__0}! {__1}".to_string(), to_set(&["__0", "__1"]),)
        );

        assert_eq!(
            parse_internal("Hello, {}! {}"),
            ("Hello, {__0}! {__1}".to_string(), to_set(&["__0", "__1"]),)
        );

        assert_eq!(
            parse_internal("Hello, {}! {} {name} {0} {} {1} {1}"),
            (
                "Hello, {__0}! {__1} {name} {__0} {__2} {__1} {__1}".to_string(),
                to_set(&["__0", "__1", "name", "__0", "__2", "__1", "__1"]),
            )
        );

        assert_eq!(
            parse_internal(
                "Hello, {:?}! {:#?} \
                {name:?} {name:#?} \
                {:b} {0:b} {0:#b} \
                {:e} {1:e} \
                {:x} {1:x} {1:#x} \
                {:o} {:#o} {1:o} {1:#o} \
                {:p} {:#p} {1:p} {1:#p} \
                {:#E} {1:#E} \
                {:x} {1:x} \
                {:X} {:#X} {1:X} {1:#X} \
                {}{} {name:?}{:b}Hello{}"
            ),
            (
                "Hello, {__0:?}! {__1:#?} \
            {name:?} {name:#?} \
            {__2:b} {__0:b} {__0:#b} \
            {__3:e} {__1:e} \
            {__4:x} {__1:x} {__1:#x} \
            {__5:o} {__6:#o} {__1:o} {__1:#o} \
            {__7:p} {__8:#p} {__1:p} {__1:#p} \
            {__9:#E} {__1:#E} \
            {__10:x} {__1:x} \
            {__11:X} {__12:#X} {__1:X} {__1:#X} \
            {__13}{__14} {name:?}{__15:b}Hello{__16}"
                    .to_string(),
                to_set(&[
                    "__0", "__1", "name", "name", "__2", "__0", "__0", "__3", "__1", "__4", "__1",
                    "__1", "__5", "__6", "__1", "__1", "__7", "__8", "__1", "__1", "__9", "__1",
                    "__10", "__1", "__11", "__12", "__1", "__1", "__13", "__14", "name", "__15",
                    "__16",
                ])
            )
        );
    }
}
