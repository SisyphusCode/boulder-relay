//! Extension traits for [`syn`]

use syn::parse::{Parse, ParseBuffer, ParseStream};
use syn::LitBool;

use private::Sealed;

use crate::{DelimitedIter, ValueSyntax};

/// [`ParseBuffer`] extensions
pub trait ParseBufferExt: Sealed {
    /// Parse a boolean attribute
    ///
    /// | Value             | Result |
    /// | ----------------- | ------- |
    /// | `my_attr`         | `true`  |
    /// | `my_attr(true)`   | `true`  |
    /// | `my_attr(false)`  | `false` |
    /// | `my_attr = true`  | `true`  |
    /// | `my_attr = false` | `false` |
    fn parse_bool_attr(&self) -> syn::Result<bool>;

    /// Parse a valued attribute
    ///
    /// | Value                 | Result                |
    /// | --------------------- | --------------------- |
    /// | `my_attr(something)`  | `P::parse(something)` |
    /// | `my_attr = something` | `P::parse(something)` |
    ///
    /// The `my_attr(something)` syntax should be preferred as `my_attr = something` can't always
    /// deserialise some types (e.g. [`Visibility`](syn::Visibility))
    fn parse_valued_attr<P: Parse>(&self) -> syn::Result<P>;

    /// Shortcut for [`DelimitedIter::new`]
    fn iter_delimited<T, D>(&self) -> DelimitedIter<T, D>
    where
        T: Parse,
        D: Parse;
}

impl<'a> ParseBufferExt for ParseBuffer<'a> {
    fn parse_bool_attr(&self) -> syn::Result<bool> {
        Ok(if let Some(syntax) = ValueSyntax::from_stream(self) {
            syntax.parse::<LitBool>(self)?.value
        } else {
            true
        })
    }

    fn parse_valued_attr<P: Parse>(&self) -> syn::Result<P> {
        ValueSyntax::from_stream(self).and_parse(self)
    }

    fn iter_delimited<T, D>(&self) -> DelimitedIter<T, D>
    where
        T: Parse,
        D: Parse,
    {
        DelimitedIter::new(self)
    }
}
impl<'a> Sealed for ParseBuffer<'a> {}

/// [`Option`] extensions
pub trait OptionExt: Sealed {
    /// If the `Option<ValueSyntax>` is `Some`, parse it based on the [`ValueSyntax`],
    /// otherwise just parse it.
    fn and_parse<P: Parse>(self, input: ParseStream) -> syn::Result<P>;
}

impl OptionExt for Option<ValueSyntax> {
    fn and_parse<P: Parse>(self, input: ParseStream) -> syn::Result<P> {
        if let Some(syntax) = self {
            syntax.parse(input)
        } else {
            input.parse()
        }
    }
}
impl Sealed for Option<ValueSyntax> {}

mod private {
    pub trait Sealed {}
}
