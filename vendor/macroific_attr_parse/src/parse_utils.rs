use cfg_if::cfg_if;
use proc_macro2::{Ident, TokenStream};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Meta, Token};

use crate::__attr_parse_prelude::*;
use crate::{DelimitedIter, ParseWrapper, ValueSyntax};

/// Get the ident from a path or [`Err`] trying
pub fn get_attr_ident(path: &syn::Path) -> syn::Result<&Ident> {
    if let Some(ident) = path.get_ident() {
        Ok(ident)
    } else {
        Err(syn::Error::new_spanned(path, "expected ident"))
    }
}

pub enum MetaValue {
    Expr(syn::Expr),
    Stream(TokenStream),
}

pub type MetaValueTuple = (Ident, Option<MetaValue>);

/// Iterate over metadata for the `ParseOption` derive macro
pub fn iterate_option_meta(
    parse: ParseStream,
) -> syn::Result<impl Iterator<Item = syn::Result<MetaValueTuple>> + '_> {
    fn map_meta(meta: Meta) -> syn::Result<MetaValueTuple> {
        Ok(match meta {
            Meta::Path(path) => (get_attr_ident(&path)?.clone(), None),
            Meta::NameValue(meta) => (
                get_attr_ident(&meta.path)?.clone(),
                Some(MetaValue::Expr(meta.value)),
            ),
            Meta::List(meta) => (
                get_attr_ident(&meta.path)?.clone(),
                Some(MetaValue::Stream(meta.tokens)),
            ),
        })
    }

    let parse_from: DelimitedIter<Meta, Token![,]> =
        if let Some(syntax) = ValueSyntax::from_stream(parse) {
            if let Some(buffer) = syntax.parse_token(parse)? {
                buffer.into()
            } else {
                parse.into()
            }
        } else {
            parse.into()
        };

    Ok(parse_from.map(move |meta| meta.and_then(map_meta)))
}

macro_rules! check_option {
    ($option: ident, $source: ident) => {
        if $option.is_some() {
            return Err(syn::Error::new($source.span(), "duplicate attribute"));
        }
    };
}

/// Decode a field while iterating attributes
pub fn decode_parse_option_field<O: ParseOption + FromExpr>(
    option: &mut Option<O>,
    ident: Ident,
    value_source: Option<MetaValue>,
) -> syn::Result<()> {
    check_option!(option, ident);

    let new_value = if let Some(meta_value) = value_source {
        match meta_value {
            MetaValue::Expr(expr) => O::from_expr(expr)?,
            MetaValue::Stream(stream) => ParseWrapper::<O>::parse_stream_self(stream)?,
        }
    } else if let Some(v) = O::boolean() {
        v
    } else {
        return Err(syn::Error::new_spanned(ident, "expected a value"));
    };

    *option = Some(new_value);

    Ok(())
}

/// Decode a field while iterating attributes
pub fn decode_attr_options_field<O>(
    option: &mut Option<O>,
    source: &impl Spanned,
    stream: ParseStream,
) -> syn::Result<()>
where
    O: ParseOption,
{
    check_option!(option, source);

    *option = Some(O::from_stream(stream)?);
    Ok(())
}

/// Decode a [`ParseOption`] with the `from_parse` option set
pub fn decode_parse_option_from_parse<O: Parse>(stream: ParseStream) -> syn::Result<O> {
    ValueSyntax::from_stream(stream).and_parse(stream)
}

/// Stable Rust `.try_collect()` implementation
pub fn try_collect<F, T, E>(iter: impl IntoIterator<Item = Result<T, E>>) -> Result<F, E>
where
    F: FromIterator<T>,
{
    cfg_if! {
        if #[cfg(feature = "nightly")] {
            iter.into_iter().try_collect()
        } else {
            struct TryCollect<'a, I, E> {
                source: I,
                error: &'a mut Option<E>,
            }
            impl<'a, T, E, I: Iterator<Item = Result<T, E>>> Iterator for TryCollect<'a, I, E> {
                type Item = T;

                fn next(&mut self) -> Option<Self::Item> {
                    if self.error.is_some() {
                        return None;
                    }

                    match self.source.next()? {
                        Ok(item) => Some(item),
                        Err(error) => {
                            *self.error = Some(error);
                            None
                        }
                    }
                }

                fn size_hint(&self) -> (usize, Option<usize>) {
                    (0, self.source.size_hint().1)
                }
            }

            let mut error = None;

            let collection: F = TryCollect {
                source: iter.into_iter(),
                error: &mut error,
            }
            .collect();

            if let Some(error) = error {
                Err(error)
            } else {
                Ok(collection)
            }
        }
    }
}
