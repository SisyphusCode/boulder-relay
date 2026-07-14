use super::FieldOptions;
use macroific::prelude::*;
use proc_macro2::Ident;
use syn::spanned::Spanned;
use syn::{Attribute, Field};

#[cfg_attr(feature = "_debug", derive(Debug))]
pub struct ParsedField {
    pub comments: Vec<Attribute>,
    pub opts: FieldOptions,
    pub ident: Ident,
    pub ty: syn::Type,
}

impl TryFrom<Field> for ParsedField {
    type Error = syn::Error;

    fn try_from(field: Field) -> syn::Result<Self> {
        let mut comments = Vec::new();
        let span = field.span();

        let attr_iter = field.attrs.into_iter().filter_map(|a| {
            let ident = a.path().get_ident()?.to_string();
            match ident.as_str() {
                "doc" => {
                    comments.push(a);
                    None
                }
                super::ATTR_NAME => Some(a),
                _ => None,
            }
        });

        let opts = FieldOptions::from_iter(span, attr_iter)?;

        Ok(Self {
            comments,
            opts,
            ident: field.ident.unwrap(),
            ty: field.ty,
        })
    }
}
