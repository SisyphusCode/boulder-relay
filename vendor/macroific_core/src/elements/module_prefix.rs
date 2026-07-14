use std::fmt;
use std::iter::Copied;
use std::ops::{Deref, Index};

use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, TokenStreamExt};
use syn::token::PathSep;

use crate::core_ext::*;

/// A module prefix, e.g. `::your_crate::__private`.
///
/// ```
/// # use macroific_core::elements::*;
/// # use proc_macro2::*;
/// # use syn::*;
/// # use quote::quote;
///
/// // `Display` implementation comes from `ToTokens`
/// const PREFIX: ModulePrefix = ModulePrefix::new(&["foo", "bar"]);
///
/// assert_eq!(PREFIX.to_string(), ":: foo :: bar");
///
/// let with_ident = PREFIX.with_ident(parse_quote!(Qux));
/// assert_eq!(with_ident.to_string(), ":: foo :: bar :: Qux");
///
/// let with_unprefixed_path = PREFIX.with_path(parse_quote!(qux::Baz));
/// let with_prefixed_path = PREFIX.with_path(parse_quote!(::qux::Baz));
///
/// assert_eq!(with_unprefixed_path.to_string(), ":: foo :: bar :: qux :: Baz");
/// assert_eq!(with_prefixed_path.to_string(), ":: foo :: bar :: qux :: Baz");
///
/// // You can chain them too
/// let chained = with_ident.with_ident(parse_quote!(Baz));
/// assert_eq!(chained.to_string(), ":: foo :: bar :: Qux :: Baz");
///
/// // They all implement ToTokens too
/// # #[allow(dead_code)]
/// let token_stream = quote! {
///   use #PREFIX;
///   use #with_ident;
///   use #with_unprefixed_path;
///   use #with_prefixed_path;
///   use #chained;
/// };
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct ModulePrefix<'a>(&'a [&'a str]);
impl ModulePrefix<'static> {
    /// Prefix for [`Option`]
    pub const OPTION: Self = Self::new(&["core", "option", "Option"]);

    /// Prefix for [`Result`]
    pub const RESULT: Self = Self::new(&["core", "result", "Result"]);
}

macro_rules! common_impl {
    () => {
        /// Attach an [`Ident`] to the end. See [`struct level docs`](ModulePrefix).
        #[must_use]
        pub const fn with_ident(&self, ident: Ident) -> ModulePrefixSuffixed<&Self, Ident> {
            self.suffixed(true, ident)
        }

        /// Attach a [`Path`](syn::Path) to the end. See [`struct level docs`](ModulePrefix).
        #[must_use]
        pub const fn with_path(&self, path: syn::Path) -> ModulePrefixSuffixed<&Self, syn::Path> {
            self.suffixed(path.leading_colon.is_none(), path)
        }
    };
}

impl<'a> ModulePrefix<'a> {
    /// Create a new `ModulePrefix` from a slice of segments.
    #[inline]
    #[must_use]
    pub const fn new(segments: &'a [&'a str]) -> Self {
        Self(segments)
    }

    common_impl!();

    #[inline]
    const fn suffixed<T>(&self, add_separator: bool, suffix: T) -> ModulePrefixSuffixed<&Self, T> {
        ModulePrefixSuffixed {
            prefix: self,
            add_separator,
            suffix,
        }
    }
}

impl<'a> fmt::Display for ModulePrefix<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.to_token_stream(), f)
    }
}

impl<'a> IntoIterator for &ModulePrefix<'a> {
    type Item = &'a str;
    type IntoIter = Copied<core::slice::Iter<'a, &'a str>>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().copied()
    }
}

impl<'a> ToTokens for ModulePrefix<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let sep = PathSep::default();

        for segment in self {
            sep.to_tokens(tokens);
            tokens.append(Ident::create(segment));
        }
    }
}

impl<'a> Deref for ModulePrefix<'a> {
    type Target = [&'a str];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a> Index<usize> for ModulePrefix<'a> {
    type Output = str;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        self.0[index]
    }
}

/// A [`ModulePrefix`] with an [`Ident`] or [`Path`](syn::Path) attached.
#[derive(Debug)]
pub struct ModulePrefixSuffixed<P, T> {
    prefix: P,
    add_separator: bool,
    suffix: T,
}

impl<P, T> ModulePrefixSuffixed<P, T> {
    common_impl!();

    #[inline]
    const fn suffixed<O>(&self, add_separator: bool, suffix: O) -> ModulePrefixSuffixed<&Self, O> {
        ModulePrefixSuffixed {
            prefix: self,
            add_separator,
            suffix,
        }
    }
}

impl<P, T> fmt::Display for ModulePrefixSuffixed<P, T>
where
    Self: ToTokens,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.to_token_stream(), f)
    }
}

impl<P: ToTokens, T: ToTokens> ToTokens for ModulePrefixSuffixed<P, T> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.prefix.to_tokens(tokens);
        if self.add_separator {
            PathSep::default().to_tokens(tokens);
        }
        self.suffix.to_tokens(tokens);
    }
}
