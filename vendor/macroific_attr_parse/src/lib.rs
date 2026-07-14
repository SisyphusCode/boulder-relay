//! Attribute parsing utilities for the [macroific](https://crates.io/crates/macroific) crate

#![cfg_attr(feature = "nightly", feature(iterator_try_collect))]
#![deny(clippy::correctness, clippy::suspicious)]
#![warn(clippy::complexity, clippy::perf, clippy::style, clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::wildcard_imports,
    clippy::default_trait_access,
    clippy::missing_errors_doc
)]
#![warn(missing_docs)]
#![cfg_attr(doc_cfg, feature(doc_cfg, doc_auto_cfg))]

use proc_macro2::{Ident, Span};
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{AttrStyle, MacroDelimiter, Meta, MetaList, PathArguments, PathSegment};

pub use delimited_iter::DelimitedIter;
pub use parse_wrapper::ParseWrapper;
pub use value_syntax::ValueSyntax;

pub use field_opt::{FieldWithOpts, FieldsWithOpts};

mod value_syntax;

pub mod ext;
mod parse_option_impls;
#[doc(hidden)]
mod parse_utils;
mod parse_wrapper;

mod delimited_iter;
mod field_opt;

/// Options derivable from [`Attributes`](syn::Attribute).
pub trait AttributeOptions: Sized {
    /// Parse and construct from an attribute
    fn from_attr(attribute: syn::Attribute) -> syn::Result<Self> {
        Self::from_iter(attribute.span(), Some(attribute))
    }

    /// Shorthand for filtering attributes by name and passing them on to
    /// [`from_iter`](Self::from_iter).
    ///
    /// The `span` is what will be used for printing errors if nothing more appropriate is
    /// available. It's likely the field or struct you're parsing.
    fn from_iter_named(
        attr_name: &str,
        span: Span,
        attributes: impl IntoIterator<Item = syn::Attribute>,
    ) -> syn::Result<Self> {
        Self::from_iter(
            span,
            attributes
                .into_iter()
                .filter(move |a| a.path().is_ident(attr_name)),
        )
    }

    /// Parse and construct from an iterator of attributes
    ///
    /// The `span` is what will be used for printing errors if nothing more appropriate is
    /// available. It's likely the field or struct you're parsing.
    fn from_iter(
        span: Span,
        attributes: impl IntoIterator<Item = syn::Attribute>,
    ) -> syn::Result<Self>;

    /// Parse a stream containing options: `opt1(val1), opt2(val2)`
    fn from_stream(input: ParseStream) -> syn::Result<Self> {
        Self::from_attr(syn::Attribute {
            pound_token: Default::default(),
            style: AttrStyle::Outer,
            bracket_token: Default::default(),
            meta: Meta::List(MetaList {
                path: syn::Path {
                    leading_colon: None,
                    segments: {
                        let mut segments = Punctuated::new();
                        segments.push_value(PathSegment {
                            ident: Ident::new("x", Span::call_site()),
                            arguments: PathArguments::None,
                        });
                        segments
                    },
                },
                delimiter: MacroDelimiter::Paren(Default::default()),
                tokens: input.parse()?,
            }),
        })
    }
}

/// Makes a type usable for [`AttributeOptions`]
pub trait ParseOption: Sized {
    /// Parses the type from the given [`ParseStream`].
    fn from_stream(input: ParseStream) -> syn::Result<Self>;
}

/// Construct this type from an [`Expr`](syn::Expr).
pub trait FromExpr: Sized {
    #[allow(missing_docs)]
    fn from_expr(expr: syn::Expr) -> syn::Result<Self>;

    /// Construct a positive boolean representation
    #[inline]
    #[must_use]
    fn boolean() -> Option<Self> {
        None
    }
}

#[doc(hidden)]
pub mod __attr_parse_prelude {
    pub use crate::ext::*;
    pub use crate::{AttributeOptions, FromExpr, ParseOption};
}

#[doc(hidden)]
pub mod __private {
    pub use crate::parse_utils::{
        decode_attr_options_field, decode_parse_option_field, decode_parse_option_from_parse,
        get_attr_ident, iterate_option_meta, try_collect, MetaValue,
    };
}
