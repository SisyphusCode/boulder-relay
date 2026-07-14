use proc_macro2::{Delimiter, Literal};
use syn::spanned::Spanned;
use syn::{Data, FieldsNamed};

use macroific_attr_parse::__attr_parse_prelude::*;
use macroific_attr_parse::__private::try_collect;

use super::super::ATTR_NAME;
use super::FieldOpts;

pub struct Field {
    pub ident: proc_macro2::Ident,
    pub opts: FieldOpts,
}

pub enum Fields {
    Unit,
    Empty(Delimiter),
    Named(Vec<Field>),
}

impl TryFrom<Data> for Fields {
    type Error = syn::Error;

    fn try_from(data: Data) -> syn::Result<Self> {
        match data {
            Data::Struct(s) => match s.fields {
                syn::Fields::Named(f) => {
                    if f.named.is_empty() {
                        Ok(Self::Empty(Delimiter::Brace))
                    } else {
                        Self::from_named(f)
                    }
                }
                syn::Fields::Unnamed(f) => {
                    if f.unnamed.is_empty() {
                        Ok(Self::Empty(Delimiter::Parenthesis))
                    } else {
                        Err(syn::Error::new_spanned(f, "Tuple structs not supported"))
                    }
                }
                syn::Fields::Unit => Ok(Self::Unit),
            },
            Data::Enum(e) => Err(syn::Error::new_spanned(e.enum_token, "Enums not supported")),
            Data::Union(u) => Err(syn::Error::new_spanned(
                u.union_token,
                "Unions not supported",
            )),
        }
    }
}

impl Fields {
    fn from_named(FieldsNamed { named: fields, .. }: FieldsNamed) -> syn::Result<Self> {
        let iter = fields.into_iter().map(move |field| -> syn::Result<Field> {
            Ok(Field {
                opts: FieldOpts::from_iter_named(ATTR_NAME, field.span(), field.attrs)?,
                ident: if let Some(ident) = field.ident {
                    ident
                } else {
                    unreachable!();
                },
            })
        });

        Ok(Self::Named(try_collect(iter)?))
    }
}

impl Field {
    pub fn resolved_label(&self) -> Literal {
        if let Some(ref rename) = self.opts.rename {
            rename.token()
        } else {
            Literal::string(&self.ident.to_string())
        }
    }
}
