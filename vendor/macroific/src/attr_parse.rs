//! Utilities for parsing [`Attribute`](syn::Attribute)s.
//!
//! # Examples
//!
//! <details><summary>Basic usage</summary>
//!
//! ```
//! use macroific::attr_parse::prelude::*;
//! # use syn::punctuated::Punctuated;
//! # use proc_macro2::{Delimiter, Span};
//! # use quote::ToTokens;
//!
//! #[derive(AttributeOptions)]
//! struct MyOptions {
//!   str_option: String,
//!   optional: Option<syn::Lifetime>,
//!   bool_option1: bool,
//!   bool_option2: bool,
//!   bool_option3: bool,
//!   a_list: Punctuated<u32, syn::Token![,]>,
//!   a_path: Option<syn::Path>,
//! }
//!
//! // You'd normally get this from `DeriveInput`
//! let attributes: Vec<syn::Attribute> = vec![
//!   syn::parse_quote! { #[my_opts(str_option = "hello", bool_option2, bool_option3 = false)] },
//!   syn::parse_quote! { #[my_opts(a_list(10, 20, 30), a_path(std::fs::File))] }
//! ];
//!
//! let opts = MyOptions::from_iter_named("my_opts", Span::call_site(), attributes).unwrap();
//!
//! // Strings & numbers can be converted straight from syn types
//! assert_eq!(opts.str_option, "hello");
//!
//! // Optionals are optional (:
//! assert!(opts.optional.is_none());
//!
//! // We didn't provide bool1 so it `Default::default()`ed to false
//! assert!(!opts.bool_option1);
//!
//! // Booleans can be provided with just the property name so bool2 is true
//! assert!(opts.bool_option2);
//!
//! // Though we can provide an explicit value too
//! assert!(!opts.bool_option3);
//!
//! // Punctuated lists are supported as the enclosed type implements ParseOption
//! assert_eq!(opts.a_list[0], 10);
//! assert_eq!(opts.a_list[1], 20);
//! assert_eq!(opts.a_list[2], 30);
//!
//! // The path is provided with an alternative syntax as an example, but it could've just as
//! // easily been provided as `a_path = std::fs::File`
//! assert_eq!(opts.a_path.unwrap().to_token_stream().to_string(), "std :: fs :: File");
//! ```
//!
//! </details>
//!
//! <details><summary>Renaming & default values</summary>
//!
//! ```
//! use macroific::attr_parse::prelude::*;
//! # use syn::{parse_quote, Attribute};
//!
//! #[derive(AttributeOptions, Debug)]
//! struct MyOptions {
//!   #[attr_opts(
//!     rename = "A",
//!     default = false // fail if this attr is not provided
//!   )]
//!   num1: u8,
//!
//!   #[attr_opts(default = some_module::default_num)] // use this function for the default value
//!   num2: u8,
//! }
//!
//! mod some_module {
//!   pub(super) fn default_num() -> u8 { u8::MAX }
//! }
//!
//! let opts = MyOptions::from_attr(parse_quote! { #[foo_attr(A = 10)] }).unwrap();
//! assert_eq!(opts.num1, 10);
//! assert_eq!(opts.num2, u8::MAX);
//!
//! let err = MyOptions::from_attr(parse_quote! { #[foo_attr()] }).unwrap_err();
//! assert_eq!(err.to_string(), r#"Missing required attribute: "A""#);
//! ```
//!
//! Full table on supported syntaxes for providing option values can be found
//! on [`parse_bool_attr`](ext::ParseBufferExt::parse_bool_attr) and
//! [`parse_valued_attr`](ext::ParseBufferExt::parse_valued_attr). See the
//! [derive macro](macroific_macro::AttributeOptions) doc page for options you can pass to it.
//!
//! </details>
//!
//! <details><summary>Nesting structs</summary>
//!
//! Nesting structs can be achieved by deriving the [`ParseOption`] trait which uses the same
//! options as `AttributeOptions`.
//!
//! ```
//! # use macroific::attr_parse::prelude::*;
//! # use syn::{parse_quote, Attribute};
//! #[derive(ParseOption, Debug, Eq, PartialEq)]
//! struct Nested {
//!   required: bool,
//!   foo: String,
//!   count: Option<u8>,
//! }
//!
//! #[derive(AttributeOptions, Debug, Eq, PartialEq)]
//! struct Options {
//!   #[attr_opts(default = false)]
//!   nest: Nested,
//!
//!   #[attr_opts(default = false)]
//!   alt: Nested,
//!
//!   root: String,
//! }
//!
//! let opts = Options::from_attr(parse_quote! { #[some_attr(root = "^", nest(count = 5, required), alt(foo = "Bar"))] })
//!   .unwrap();
//!
//! let expect = Options {
//!  nest: Nested { required: true, count: Some(5), foo: String::new() },
//!  alt: Nested { required: false, count: None, foo: "Bar".into() },
//!  root: "^".into(),
//! };
//!
//! assert_eq!(opts, expect);
//! ```
//!
//! </details>
//!
//! # Features
//!
//! Enable the `full` feature to implement [`ParseOption`] for syn types that require it.

pub use macroific_attr_parse::*;
#[cfg(feature = "derive")]
pub use macroific_macro::{AttributeOptions, ParseOption};

#[allow(missing_docs)]
pub mod prelude {
    #[cfg(feature = "derive")]
    pub use macroific_macro::{AttributeOptions, ParseOption};

    pub use super::__attr_parse_prelude::*;
}
