use std::ops::{Deref, DerefMut};

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{token, Attribute, Field, Fields, FieldsNamed, FieldsUnnamed, Token};

use crate::parse_utils::try_collect;
use crate::AttributeOptions;

/// A [`Field`] with options [parsed](AttributeOptions).
pub struct FieldWithOpts<O> {
    #[allow(missing_docs)]
    pub field: Field,

    #[allow(missing_docs)]
    pub options: O,
}

/// [`Fields`] with options [parsed](AttributeOptions).
pub enum FieldsWithOpts<O> {
    #[allow(missing_docs)]
    Named {
        brace_token: token::Brace,
        fields: Punctuated<FieldWithOpts<O>, Token![,]>,
    },

    #[allow(missing_docs)]
    Unnamed {
        paren_token: token::Paren,
        fields: Punctuated<FieldWithOpts<O>, Token![,]>,
    },

    #[allow(missing_docs)]
    Unit,
}

macro_rules! from_attr_name {
    ($input: ty) => {
        /// [`from_predicate`](Self::from_predicate) shorthand for filtering by attribute name
        pub fn from_attr_name(input: $input, attr_name: &str) -> ::syn::Result<Self> {
            Self::from_predicate(input, move |attr| attr.path().is_ident(attr_name))
        }
    };
}

impl<O: AttributeOptions> FieldWithOpts<O> {
    /// Construct from the given field using attributes the predicate returns true for. The remaining attributes will
    /// be kept on the field.
    pub fn from_predicate<F>(mut field: Field, mut predicate: F) -> syn::Result<Self>
    where
        F: FnMut(&Attribute) -> bool,
    {
        let mut relevant_attrs = Vec::new();
        field.attrs = field
            .attrs
            .into_iter()
            .filter_map(|attr| {
                if predicate(&attr) {
                    relevant_attrs.push(attr);
                    None
                } else {
                    Some(attr)
                }
            })
            .collect();

        Ok(Self {
            options: O::from_iter(field.span(), relevant_attrs)?,
            field,
        })
    }

    from_attr_name!(Field);
}

impl<O: AttributeOptions> FieldsWithOpts<O> {
    /// Construct from the given fields using attributes the predicate returns true for. The remaining attributes will
    /// be kept on the field.
    pub fn from_predicate<F>(fields: Fields, mut predicate: F) -> syn::Result<Self>
    where
        F: FnMut(&Attribute) -> bool,
    {
        match fields {
            Fields::Named(FieldsNamed { brace_token, named }) => {
                let iter = named
                    .into_iter()
                    .map(|field| FieldWithOpts::from_predicate(field, &mut predicate));

                Ok(Self::Named {
                    fields: try_collect(iter)?,
                    brace_token,
                })
            }
            Fields::Unnamed(FieldsUnnamed {
                paren_token,
                unnamed,
            }) => {
                let fields = unnamed
                    .into_iter()
                    .map(|field| FieldWithOpts::from_predicate(field, &mut predicate));

                Ok(Self::Unnamed {
                    fields: try_collect(fields)?,
                    paren_token,
                })
            }
            Fields::Unit => Ok(Self::Unit),
        }
    }

    from_attr_name!(Fields);
}

impl<O> ToTokens for FieldsWithOpts<O> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match *self {
            Self::Named {
                ref brace_token,
                ref fields,
            } => {
                brace_token.surround(tokens, move |tokens| fields.to_tokens(tokens));
            }
            Self::Unnamed {
                ref paren_token,
                ref fields,
            } => {
                paren_token.surround(tokens, move |tokens| fields.to_tokens(tokens));
            }
            Self::Unit => {}
        }
    }
}

impl<O> ToTokens for FieldWithOpts<O> {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.field.to_tokens(tokens);
    }

    #[inline]
    fn to_token_stream(&self) -> TokenStream {
        self.field.to_token_stream()
    }

    #[inline]
    fn into_token_stream(self) -> TokenStream {
        self.field.into_token_stream()
    }
}

impl<O> Deref for FieldWithOpts<O> {
    type Target = Field;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl<O> DerefMut for FieldWithOpts<O> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.field
    }
}
