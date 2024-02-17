use std::collections::BTreeMap;

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
    pub text: String,

    /// Identifiers used in the interpolated text.
    ///
    /// We also keep any traits that are used in the interpolated text. Basically, anything that
    /// after the `:` in the interpolated text. If there's trait specified, the trait value is `None`.
    pub used_identifiers: BTreeMap<String, Option<String>>,
}

impl Interpolate<'_> {
    /// Parse the format text and extract the fields to be interpolated.
    /// Returns a tuple of the fields and the format string with the interpolated fields replaced with
    /// the __ prefix (and for positional values, __0, __1, etc.)
    pub fn parse<'a>(fmt_text: impl AsRef<str>, variant: &'a Variant) -> Interpolate<'a> {
        let (text, used_identifiers) = parse_internal(fmt_text);

        Interpolate {
            variant,
            text,
            used_identifiers,
        }
    }
}

/// Parse the format text and extract the identifiers to be interpolated.
fn parse_internal(fmt_text: impl AsRef<str>) -> (String, BTreeMap<String, Option<String>>) {
    let mut used_identifiers = BTreeMap::new();
    let mut fmt_string = String::new();
    let mut chars = fmt_text.as_ref().chars().peekable();
    let mut index_for_unnamed = -1;

    while let Some(c) = chars.next() {
        if c != '{' {
            fmt_string.push(c);
            continue;
        }

        // If the next character is also a '{', then it's an escaped '{'
        if let Some('{') = chars.peek() {
            fmt_string.push_str("{{");
            chars.next();
            continue;
        }

        let mut field = "".to_string();
        let mut traits = None;
        while let Some(c) = chars.next() {
            if c == ':' {
                // If no field name was parsed bfore the ':', then it's a positional value;
                // so we need to add the index to the field name
                if field.is_empty() {
                    index_for_unnamed += 1;
                    field.push_str(&format!("__{}", index_for_unnamed));
                }

                if field.parse::<usize>().is_ok() {
                    field = format!("__{}", field);
                }

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
                // If no field name was parsed, then it's a positional value
                if field.is_empty() {
                    index_for_unnamed += 1;
                    field.push_str(&format!("__{}", index_for_unnamed));
                }

                if field.parse::<u8>().is_ok() {
                    field = format!("__{}", field);
                }

                fmt_string.push_str(&format!(
                    "{{{}{}}}",
                    &field,
                    traits.as_ref().map(|c| format!(":{c}")).unwrap_or_default()
                ));

                used_identifiers.insert(field.clone(), traits);
                break;
            }

            field.push(c);
        }
    }

    (fmt_string, used_identifiers)
}

#[cfg(feature = "display")]
impl quote::ToTokens for Interpolate<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let variant_name = &self.variant.ident;
        let interpolated_text = &self.text;

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
                        .and_then(|ident| build_ident_assignment(ident, &self.used_identifiers))
                });

                let fields_ident = self
                    .used_identifiers
                    .iter()
                    .map(|(ident, _)| Ident::new(ident, proc_macro2::Span::call_site()));

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
    used_fields: &BTreeMap<String, Option<String>>,
) -> Option<proc_macro2::TokenStream> {
    use quote::format_ident;

    // If the field is not present in the format string, then we don't need to interpolate it
    if !used_fields.contains_key(&ident.to_string()) {
        return None;
    }

    let ident = format_ident!("__{}", ident);
    Some(quote! { #ident = self.#ident })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::parse_internal;

    fn to_map<T: ToString>(values: &[(T, Option<T>)]) -> BTreeMap<String, Option<String>> {
        values
            .iter()
            .map(|(a, b)| (a.to_string(), b.as_ref().map(|s| s.to_string())))
            .collect()
    }

    #[test]
    fn test_parse_fmt_string() {
        assert_eq!(
            parse_internal("Hello, {name}!"),
            ("Hello, {name}!".to_string(), to_map(&[("name", None)]),)
        );

        assert_eq!(
            parse_internal("Hello, {name}! {age}"),
            (
                "Hello, {name}! {age}".to_string(),
                to_map(&[("name", None), ("age", None)]),
            )
        );

        assert_eq!(
            parse_internal("Hello, {0}! {1}"),
            (
                "Hello, {__0}! {__1}".to_string(),
                to_map(&[("__0", None), ("__1", None)]),
            )
        );

        assert_eq!(
            parse_internal("Hello, {}! {}"),
            (
                "Hello, {__0}! {__1}".to_string(),
                to_map(&[("__0", None), ("__1", None)]),
            )
        );

        assert_eq!(
            parse_internal("Hello, {}! {} {name} {0} {} {1} {1}"),
            (
                "Hello, {__0}! {__1} {name} {__0} {__2} {__1} {__1}".to_string(),
                to_map(&[
                    ("__0", None),
                    ("__1", None),
                    ("name", None),
                    ("__0", None),
                    ("__2", None),
                    ("__1", None),
                    ("__1", None),
                ]),
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
                to_map(&[
                    ("__0", Some("?")),
                    ("__1", Some("#?")),
                    ("name", Some("?")),
                    ("name", Some("#?")),
                    ("__2", Some("b")),
                    ("__0", Some("b")),
                    ("__0", Some("#b")),
                    ("__3", Some("e")),
                    ("__1", Some("e")),
                    ("__4", Some("x")),
                    ("__1", Some("x")),
                    ("__1", Some("#x")),
                    ("__5", Some("o")),
                    ("__6", Some("#o")),
                    ("__1", Some("o")),
                    ("__1", Some("#o")),
                    ("__7", Some("p")),
                    ("__8", Some("#p")),
                    ("__1", Some("p")),
                    ("__1", Some("#p")),
                    ("__9", Some("#E")),
                    ("__1", Some("#E")),
                    ("__10", Some("x")),
                    ("__1", Some("x")),
                    ("__11", Some("X")),
                    ("__12", Some("#X")),
                    ("__1", Some("X")),
                    ("__1", Some("#X")),
                    ("__13", None),
                    ("__14", None),
                    ("name", Some("?")),
                    ("__15", Some("b")),
                    ("__16", None),
                ])
            )
        );
    }
}
