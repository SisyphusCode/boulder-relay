use super::DefaultOption;
use macroific_attr_parse::__attr_parse_prelude::*;
use macroific_attr_parse::__private::{decode_attr_options_field, get_attr_ident};
use proc_macro2::Span;
use quote::ToTokens;
use syn::{Attribute, LitStr};

pub struct FieldOpts {
    pub default: Option<DefaultOption>,
    pub rename: Option<LitStr>,
}

impl FieldOpts {
    pub fn omit_default(&self) -> bool {
        matches!(self.default, Some(DefaultOption::Explicit(false)))
    }
}

impl AttributeOptions for FieldOpts {
    fn from_iter(_: Span, attrs: impl IntoIterator<Item = Attribute>) -> syn::Result<Self> {
        let mut default = None;
        let mut rename = None;

        for attr in attrs {
            attr.parse_nested_meta(|meta| {
                let ident = get_attr_ident(&meta.path)?;

                match ident.to_string().as_str() {
                    "default" => decode_attr_options_field(&mut default, ident, meta.input),
                    "rename" => decode_attr_options_field(&mut rename, ident, meta.input),
                    other => Err(syn::Error::new_spanned(
                        ident,
                        format!("Unrecognised attribute: `{}`", other),
                    )),
                }
            })?;
        }

        Ok(Self { default, rename })
    }
}

impl std::fmt::Debug for FieldOpts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("FieldOpts");
        debug.field("default", &self.default);

        if let Some(ref str) = self.rename {
            debug.field("rename", &str.to_token_stream().to_string());
        } else {
            debug.field("rename", &None::<()>);
        }

        debug.finish()
    }
}
