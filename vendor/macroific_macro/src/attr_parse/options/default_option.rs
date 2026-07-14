use quote::ToTokens;
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseBuffer, ParseStream};
use syn::{LitBool, Token};

use macroific_attr_parse::ValueSyntax;
use macroific_attr_parse::__attr_parse_prelude::*;

pub enum DefaultOption {
    Implicit,
    Explicit(bool),
    Path(syn::Path),
}

impl Parse for DefaultOption {
    fn parse(parse: ParseStream) -> syn::Result<Self> {
        fn peek_expr(fork: &ParseBuffer) -> bool {
            if fork.peek(Token![::]) {
                fork.peek2(syn::Ident::peek_any)
            } else {
                fork.peek(syn::Ident::peek_any)
            }
        }

        fn parse_explicit(parse: &ParseBuffer) -> syn::Result<DefaultOption> {
            if parse.peek(LitBool) {
                Ok(DefaultOption::Explicit(parse.parse::<LitBool>()?.value))
            } else if peek_expr(parse) {
                Ok(DefaultOption::Path(parse.parse()?))
            } else {
                Err(parse.error("Expected boolean or path"))
            }
        }

        match ValueSyntax::from_stream(parse) {
            None => Ok(Self::Implicit),
            Some(syntax) => {
                if let Some(parse) = syntax.parse_token(parse)? {
                    parse_explicit(&parse)
                } else {
                    parse_explicit(parse)
                }
            }
        }
    }
}

impl ParseOption for DefaultOption {
    #[inline]
    fn from_stream(input: ParseStream) -> syn::Result<Self> {
        Self::parse(input)
    }
}

impl std::fmt::Debug for DefaultOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("DefaultOption");
        match self {
            Self::Implicit => debug.field("implicit", &true),
            Self::Explicit(value) => debug.field("explicit", value),
            Self::Path(path) => debug.field("path", &path.to_token_stream()),
        };

        debug.finish()
    }
}
