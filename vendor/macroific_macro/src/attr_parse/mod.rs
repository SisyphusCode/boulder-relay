use std::fmt::Write;

use proc_macro2::TokenStream;
use proc_macro2::{Delimiter, Group, Ident, Punct};
use quote::{format_ident, quote, ToTokens, TokenStreamExt};
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Attribute, DeriveInput, Generics};

pub use attr_options::AttrOptionsDerive;
use macroific_core::core_ext::{MacroificCoreIdentExt, MacroificCorePunctExt};
use macroific_core::elements::{ImplFor, ModulePrefix, SimpleAttr};
use options::*;
pub use parse_option::ParseOptionDerive;

use crate::BaseTokenStream;

macro_rules! common_impl {
    (to_tokens) => {
        #[inline]
        fn to_tokens(&self, _: &mut TokenStream) {
            super::to_tokens();
        }
    };

    ($for: ident $trait_name: literal) => {
        use super::{
            BaseTokenStream, Delimiter, Fields, Generics, Group, Ident, ParseStream, Render,
            ToTokens, TokenStream, INLINE, RESULT,
        };
        use ::syn::parse::Parse;
        use quote::{quote, TokenStreamExt};

        impl Render for $for {
            const TRAIT_NAME: &'static str = $trait_name;

            #[inline]
            fn generics(&self) -> &Generics {
                &self.generics
            }

            #[inline]
            fn ident(&self) -> &Ident {
                &self.ident
            }

            #[inline]
            fn fields(&self) -> &Fields {
                &self.fields
            }
        }

        impl $for {
            #[inline]
            pub fn run(input: BaseTokenStream) -> BaseTokenStream {
                super::run::<Self>(input)
            }

            fn render_empty(&self, delimiter: Option<Delimiter>) -> TokenStream {
                let ending = super::empty_ending(delimiter);
                let mut tokens = self.impl_for();

                tokens.append(Group::new(
                    Delimiter::Brace,
                    Self::render_empty_body(ending),
                ));

                tokens
            }
        }
    };
}

mod attr_options;
mod options;
mod parse_option;

const OPTION: ModulePrefix = ModulePrefix::OPTION;
const RESULT: ModulePrefix = ModulePrefix::RESULT;
const INLINE: SimpleAttr = SimpleAttr::INLINE;

const ATTR_NAME: &str = "attr_opts";

const PRIVATE: ModulePrefix = ModulePrefix::new(&["macroific", "attr_parse", "__private"]);
const BASE: ModulePrefix = ModulePrefix::new(&["macroific", "attr_parse"]);

trait Render {
    const TRAIT_NAME: &'static str;

    fn generics(&self) -> &Generics;
    fn ident(&self) -> &Ident;
    fn fields(&self) -> &Fields;

    #[inline]
    fn impl_for(&self) -> TokenStream {
        impl_for(self.generics(), self.ident(), Self::TRAIT_NAME)
    }

    fn named_fields(&self) -> Result<&[Field], Option<Delimiter>> {
        match *self.fields() {
            Fields::Named(ref fields) => Ok(fields),
            Fields::Empty(delim) => Err(Some(delim)),
            Fields::Unit => Err(None),
        }
    }
}

#[inline]
fn to_tokens() {
    unimplemented!("Use to_token_stream")
}

fn nones(fields: &[Field]) -> TokenStream {
    (0..fields.len())
        .map(move |idx| {
            let ident = field_ident_at(idx);
            quote! { let mut #ident = #OPTION::None; }
        })
        .collect()
}

fn unwraps<'a>(
    indexed_fields: impl Iterator<Item = IndexedFieldTuple<'a>>,
    span_arg_name: &impl ToTokens,
) -> Group {
    let body = indexed_fields.map(move |(option_var_name, field)| {
        let mut out = field.ident.to_token_stream();
        out.append(Punct::new_joint(':'));

        match field.opts.default {
            None | Some(DefaultOption::Implicit | DefaultOption::Explicit(true)) => {
                out.extend(quote! { #option_var_name.unwrap_or_default() });
            }
            Some(DefaultOption::Explicit(false)) => {
                let mut missing_field_err = String::from("Missing required attribute: ");
                if let Some(ref rename) = field.opts.rename {
                    write!(&mut missing_field_err, "{}", rename.token()).unwrap();
                } else {
                    write!(&mut missing_field_err, "{}", field.ident).unwrap();
                }

                out.extend(quote! { if let #OPTION::Some(v) = #option_var_name {
                    v
                } else {
                    return #RESULT::Err(::syn::Error::new(#span_arg_name, #missing_field_err));
                } });
            }
            Some(DefaultOption::Path(ref path)) => {
                out.extend(quote! { #option_var_name.unwrap_or_else(#path) });
            }
        };

        out.append(Punct::new_joint(','));

        out
    });

    Group::new(Delimiter::Brace, body.collect())
}

type IndexedFieldTuple<'a> = (Ident, &'a Field);

fn indexed_fields(fields: &[Field]) -> impl Iterator<Item = IndexedFieldTuple> + Clone {
    fields.iter().enumerate().map(move |(idx, field)| {
        let option_var_name = field_ident_at(idx);

        (option_var_name, field)
    })
}

fn empty_ending(delimiter: Option<Delimiter>) -> Option<Group> {
    delimiter.map(move |d| Group::new(d, TokenStream::new()))
}

fn run<T: Parse + ToTokens>(input: BaseTokenStream) -> BaseTokenStream {
    parse_macro_input!(input as T).into_token_stream().into()
}

fn impl_for(generics: &Generics, ident: &Ident, trait_name: &str) -> TokenStream {
    let impl_trait = BASE.with_ident(Ident::create(trait_name));
    let mut tokens = SimpleAttr::AUTO_DERIVED.into_token_stream();

    ImplFor::new(generics, impl_trait, ident).to_tokens(&mut tokens);

    tokens
}

fn common_construct(input: ParseStream) -> syn::Result<(Ident, Generics, Fields, Vec<Attribute>)> {
    let DeriveInput {
        ident,
        generics,
        data,
        attrs,
        ..
    } = input.parse()?;

    Ok((ident, generics, data.try_into()?, attrs))
}

fn field_ident_at(idx: usize) -> Ident {
    format_ident!("field{idx}")
}
