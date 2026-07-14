use crate::__attr_parse_prelude::*;
use syn::parse::{Parse, ParseStream};

/// A wrapper to make any [`ParseOption`] into a [`Parse`].
#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[repr(transparent)]
pub struct ParseWrapper<T>(T);

impl<T> ParseWrapper<T> {
    #[allow(missing_docs)]
    #[inline]
    pub const fn new(inner: T) -> Self {
        Self(inner)
    }

    /// Get the inner value
    #[inline]
    pub fn inner(self) -> T {
        self.0
    }

    /// Shorthand for [`parse`](Parse::parse) followed by [`inner`](ParseWrapper::inner).
    pub fn parse_self(input: ParseStream) -> syn::Result<T>
    where
        T: ParseOption,
    {
        Ok(Self::parse(input)?.inner())
    }

    /// [`parse_self`](ParseWrapper::parse_self) that accepts a [`proc_macro2::TokenStream`].
    pub fn parse_stream_self(input: proc_macro2::TokenStream) -> syn::Result<T>
    where
        T: ParseOption,
    {
        Ok(syn::parse2::<Self>(input)?.inner())
    }
}

impl<T: ParseOption> Parse for ParseWrapper<T> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self(T::from_stream(input)?))
    }
}
