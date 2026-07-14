//! Proc macros for the [macroific](https://docs.rs/macroific) crate.

#![deny(clippy::correctness, clippy::suspicious)]
#![warn(clippy::complexity, clippy::perf, clippy::style, clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::wildcard_imports,
    clippy::uninlined_format_args
)]
#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![warn(missing_docs)]

#[allow(unused_imports)]
use proc_macro::TokenStream as BaseTokenStream;

#[cfg(feature = "attr_parse")]
mod attr_parse;

/// Derive the `AttributeOptions` trait for a struct.
///
/// | Options |  |
/// | ----- | ----- |
/// | `#[attr_opts(rename = "new_ident")]` | Use this ident when parsing instead of the struct field's name |
/// | `#[attr_opts(default = some_module::default_fn)]` | Use this function for the default value |
/// | `#[attr_opts(default = false)]` | Make this option required and error if it isn't provided |
///
/// The alternative syntax is fine too, `#[attr_opts(default(false))]`
#[cfg(feature = "attr_parse")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "attr_parse")))]
#[proc_macro_derive(AttributeOptions, attributes(attr_opts))]
pub fn derive_attribute_options(input: BaseTokenStream) -> BaseTokenStream {
    attr_parse::AttrOptionsDerive::run(input)
}

/// Derive the `ParseOption` trait for a struct. Uses the same field options as [`AttributeOptions`].
///
/// | Container Options |  |
/// | ----- | ----- |
/// | `#[attr_opts(from_parse)]` | Call [`Parse::parse`](::syn::parse::Parse::parse) to implement `ParseOption`. `Parse` will also get implemented if this option is omitted or `false` |
#[cfg(feature = "attr_parse")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "attr_parse")))]
#[proc_macro_derive(ParseOption, attributes(attr_opts))]
pub fn derive_parse_option(input: BaseTokenStream) -> BaseTokenStream {
    attr_parse::ParseOptionDerive::run(input)
}
