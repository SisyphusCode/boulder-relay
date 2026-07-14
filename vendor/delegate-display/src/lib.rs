//! Lets you derive `Display` & `Debug` traits on structs with
//! `0..=1` fields & enums where each variant has `0..=1` fields - see input/output examples below.
//!
//! [![master CI badge](https://img.shields.io/github/actions/workflow/status/Alorel/delegate-display-rs/ci.yml?label=master%20CI)](https://github.com/Alorel/delegate-display-rs/actions/workflows/ci.yml?query=branch%3Amaster)
//! [![crates.io badge](https://img.shields.io/crates/v/delegate-display)](https://crates.io/crates/delegate-display)
//! [![docs.rs badge](https://img.shields.io/docsrs/delegate-display?label=docs.rs)](https://docs.rs/delegate-display)
//! [![dependencies badge](https://img.shields.io/librariesio/release/cargo/delegate-display)](https://libraries.io/cargo/delegate-display)
//!
//! # Examples
//!
//! <details><summary>Newtype structs</summary>
//!
#![cfg_attr(doctest, doc = " ````no_test")]
//! ```
//! // Input
//! #[derive(delegate_display::DelegateDisplay)]
//! struct Foo(SomeType);
//!
//! // Output
//! impl fmt::Display for Foo {
//!   #[inline]
//!   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//!     fmt::Display::fmt(&self.0, f)
//!   }
//! }
//! ````
//!
//! </details>
//!
//! <details><summary>Structs with one field</summary>
//!
#![cfg_attr(doctest, doc = " ````no_test")]
//! ```
//! // Input
//! #[derive(delegate_display::DelegateDebug)]
//! struct Foo { some_field: SomeType }
//!
//! // Output
//! impl fmt::Debug for Foo {
//!   #[inline]
//!   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//!     fmt::Debug::fmt(&self.some_field, f)
//!   }
//! }
//! ````
//!
//! </details>
//!
//! <details><summary>Enums</summary>
//!
#![cfg_attr(doctest, doc = " ````no_test")]
//! ```
//! // Input
//! enum MyEnum {
//!   Foo,
//!   Bar(SomeType),
//!   Qux { baz: SomeType }
//! }
//!
//! // Output
//! fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//!   match self {
//!     Self::Foo => f.write_str("Foo"),
//!     Self::Bar(inner) => DebugOrDisplay::fmt(inner, f),
//!     Self::Qux { baz } => DebugOrDisplay::fmt(baz, f),
//!   }
//! }
//! ````
//!
//! </details>
//!
//! <details><summary>Empty structs & enums</summary>
//!
#![cfg_attr(doctest, doc = " ````no_test")]
//! ```
//! // Input
//! struct Foo;
//! struct Bar{}
//! struct Qux();
//! enum Baz {}
//!
//! // Output
//! fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
//!   Ok(())
//! }
//! ````
//!
//! </details>
//!
//! <details><summary>Custom generic bounds</summary>
//!
//! The attribute names are `ddebug` for `Debug`, `ddisplay` for `Display` and `dboth` for a common config for
//! both. `ddebug` and `ddisplay` take precendence over `dboth`.
//!
//! - `base_bounds` will add whatever trait is being derived as a generic bound to each of the struct/enum's generic params
//! - `bounds(...)` will let you specify specific bounds
//!
#![cfg_attr(doctest, doc = " ````no_test")]
//! ```
//! // Input
//! #[derive(DelegateDisplay, DelegateDebug)]
//! #[dboth(base_bounds)]
//! #[ddisplay(bounds(F: Display, B: Clone + Display))]
//! enum Foo<F, B> {
//!   Foo(F),
//!   Bar(B),
//! }
//!
//! // Output
//! impl<F: Display, B: Clone + Display> Display for Foo<F, B> { /* ... */}
//! impl<F: Debug, B: Debug> Debug for Foo<F, B> { /* ... */ }
//! ````
//!
//! </details>
//!
//! <details><summary>Typed delegations</summary>
//!
//! Can be useful for further prettifying the output.
//!
//! ```
//! # use delegate_display::DelegateDebug;
//! /// Some type that `Deref`s to the type we want to use in our formatting, in this case, `str`.
//! #[derive(Debug)]
//! struct Wrapper(&'static str);
//! # impl std::ops::Deref for Wrapper {
//! #   type Target = str;
//! #   fn deref(&self) -> &Self::Target {
//! #     self.0
//! #   }
//! # }
//!
//! #[derive(DelegateDebug)]
//! #[ddebug(delegate_to(str))] // ignore `Wrapper` and debug the `str` it `Deref`s instead
//! struct Typed(Wrapper);
//!
//! #[derive(DelegateDebug)] // Included for comparison
//! struct Base(Wrapper);
//!
//! assert_eq!(format!("{:?}", Typed(Wrapper("foo"))), "\"foo\"");
//! assert_eq!(format!("{:?}", Base(Wrapper("bar"))), "Wrapper(\"bar\")");
//! ```
//!
//! </details>
//!
//! <details><summary>Invalid inputs</summary>
//!
//! ```compile_fail
//! # use {std::sync::Arc, delegate_display::DelegateDisplay};
//! #[derive(DelegateDisplay, Debug)]
//! #[dboth(delegate_to(String))] // `delegate_to` is not supported on enums
//! enum SomeEnum {
//!   Foo(Arc<String>)
//! }
//! ```
//!
//! ```compile_fail
//! #[derive(delegate_display::DelegateDisplay)]
//! #[ddisplay(base_bounds, bounds(T: Display))] // `base_bounds` and `bounds` are mutually exclusive
//! struct Generic<T>(T);
//! ```
//!
//! ```compile_fail
//! #[derive(delegate_display::DelegateDisplay)]
//! #[ddisplay(base_bounds)]
//! #[ddisplay(base_bounds)] // `dbodh` and `ddisplay` can be mixed, but the same option can't be used twice
//! struct Foo<T>(T);
//! ```
//!
//! ```compile_fail
//! #[derive(delegate_display::DelegateDebug)]
//! struct TooManyFields1 {
//!   foo: u8,
//!   bar: u8, // Only one field permitted
//! }
//! ```
//!
//! ```compile_fail
//! #[derive(delegate_display::DelegateDebug)]
//! struct TooManyFields2(u8, u8); // too many fields
//! ```
//!
//! ```compile_fail
//! #[derive(delegate_display::DelegateDebug)]
//! enum SomeEnum {
//!   A, // this is ok
//!   B(u8), // this is ok
//!   C { foo: u8 }, // this is ok
//!   D(u8, u8), // Only one field permitted
//!   E { foo: u8, bar: u8 } // Only one field permitted
//! }
//! ```
//!
//! ```compile_fail
//! #[derive(delegate_display::DelegateDebug)]
//! union Foo { bar: u8 } // Unions are not supported
//! ```
//!
//! </details>

#![deny(clippy::correctness, clippy::suspicious)]
#![warn(clippy::complexity, clippy::perf, clippy::style, clippy::pedantic)]
#![allow(
    clippy::wildcard_imports,
    clippy::default_trait_access,
    clippy::single_match_else
)]
#![warn(missing_docs)]

use proc_macro::TokenStream as BaseTokenStream;

mod parse;
mod tokenise;

/// Derive the [Debug](core::fmt::Debug) trait
///
/// See [crate-level documentation](crate) for information on what's acceptable and what's not.
#[proc_macro_derive(DelegateDebug, attributes(ddebug, dboth))]
#[inline]
pub fn derive_debug(tokens: BaseTokenStream) -> BaseTokenStream {
    ParsedData::process("Debug", tokens)
}

/// Derive the [Display](core::fmt::Display) trait
///
/// See [crate-level documentation](crate) for information on what's acceptable and what's not.
#[proc_macro_derive(DelegateDisplay, attributes(ddisplay, dboth))]
#[inline]
pub fn derive_display(tokens: BaseTokenStream) -> BaseTokenStream {
    ParsedData::process("Display", tokens)
}

struct ParsedData {
    ident: syn::Ident,
    generics: syn::Generics,
    first_field: FirstField,
    options: ContainerOptions,
}

enum FieldLike {
    Indexed(syn::Type),
    Ident(syn::Ident, syn::Type),
}

type EnumData = (syn::Ident, Option<Box<FieldLike>>);

enum FirstField {
    Struct(Option<Box<FieldLike>>),
    Enum(Vec<EnumData>),
}

#[derive(macroific::attr_parse::AttributeOptions, Default)]
struct ContainerOptions {
    pub bounds: syn::punctuated::Punctuated<syn::WherePredicate, syn::Token![,]>,
    pub base_bounds: bool,
    pub delegate_to: Option<syn::Type>,
}
