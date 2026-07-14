use crate::core_ext::*;
use proc_macro2::{Delimiter, Group, Ident, Punct, TokenStream};
use quote::{ToTokens, TokenStreamExt};
use std::ops::Deref;

/// A simple inner or outer attribute, e.g. `#[inline]` or `#![foo]`
#[derive(Copy, Clone)]
pub struct SimpleAttr<'a> {
    text: &'a str,
    is_inner: bool,
}

impl<'a> Deref for SimpleAttr<'a> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.text
    }
}

impl<'a> SimpleAttr<'a> {
    /// Check whether this is an [inner](syn::AttrStyle::Inner) or
    /// [outer](syn::AttrStyle::Outer) attribute
    #[inline]
    #[must_use]
    pub const fn is_inner(self) -> bool {
        self.is_inner
    }

    /// Constructor for an [inner](syn::AttrStyle::Inner) attribute
    #[inline]
    #[must_use]
    pub const fn new_inner(text: &'a str) -> Self {
        Self {
            text,
            is_inner: true,
        }
    }

    /// Constructor for an [outer](syn::AttrStyle::Outer) attribute
    #[inline]
    #[must_use]
    pub const fn new_outer(text: &'a str) -> Self {
        Self {
            text,
            is_inner: false,
        }
    }
}

impl SimpleAttr<'static> {
    /// `#[automatically_derived]`
    pub const AUTO_DERIVED: Self = Self::new_outer("automatically_derived");

    /// `#[inline]`
    pub const INLINE: Self = Self::new_outer("inline");
}

impl ToTokens for SimpleAttr<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append(Punct::new_joint('#'));
        if self.is_inner {
            tokens.append(Punct::new_joint('!'));
        }

        let body = Ident::create(self.text).into_token_stream();
        tokens.append(Group::new(Delimiter::Bracket, body));
    }
}
