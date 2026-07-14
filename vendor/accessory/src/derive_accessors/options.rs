use macroific::prelude::*;
use proc_macro2::{Ident, Punct, TokenStream};
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Error, Expr, LitStr, Token, Visibility, WherePredicate};

#[derive(AttributeOptions)]
#[cfg_attr(feature = "_debug", derive(Debug))]
pub struct ContainerOptions {
    pub get: bool,
    pub get_mut: bool,
    pub set: bool,

    pub defaults: ContainerDefaults,
    pub bounds: Punctuated<WherePredicate, Token![,]>,
}

#[derive(AttributeOptions)]
#[cfg_attr(feature = "_debug", derive(Debug))]
pub struct FieldOptions {
    pub skip: bool,
    pub all: Option<VariationOptions>,
    pub get: Option<VariationOptions>,
    pub get_mut: Option<VariationOptions>,
    pub set: Option<VariationOptions>,
}

#[derive(ParseOption, Default)]
#[cfg_attr(feature = "_debug", derive(Debug))]
pub struct ContainerDefaults {
    pub all: VariationDefaults,
    pub get: VariationDefaults,
    pub get_mut: VariationDefaults,
    pub set: VariationDefaults,
}

#[derive(ParseOption, Clone)]
#[cfg_attr(feature = "_debug", derive(Debug))]
pub struct VariationOptions {
    pub owned: Option<bool>,
    pub const_fn: Option<bool>,
    pub skip: Option<bool>,
    pub cp: Option<bool>,
    pub ptr_deref: Option<DerefKind>,
    pub ty: Option<syn::Type>,
    pub prefix: Option<SkippableIdent>,
    pub suffix: Option<SkippableIdent>,
    pub vis: Option<Visibility>,
    pub bounds: Punctuated<WherePredicate, Token![,]>,
}

#[derive(ParseOption, Default)]
#[cfg_attr(feature = "_debug", derive(Debug))]
pub struct VariationDefaults {
    pub owned: Option<bool>,
    pub const_fn: Option<bool>,
    pub cp: Option<bool>,
    pub ptr_deref: Option<DerefKind>,
    pub prefix: Option<SkippableIdent>,
    pub suffix: Option<SkippableIdent>,
    pub vis: Option<Visibility>,
    pub bounds: Punctuated<WherePredicate, Token![,]>,
}

impl FromExpr for VariationDefaults {
    fn from_expr(expr: Expr) -> syn::Result<Self> {
        Err(Error::new_spanned(
            expr,
            "VariationDefaults can't be constructed from an expression",
        ))
    }
}

impl From<&VariationDefaults> for VariationOptions {
    fn from(defaults: &VariationDefaults) -> Self {
        Self {
            owned: defaults.owned,
            const_fn: defaults.const_fn,
            skip: None,
            cp: defaults.cp,
            ptr_deref: defaults.ptr_deref,
            ty: None,
            prefix: defaults.prefix.clone(),
            suffix: defaults.suffix.clone(),
            vis: defaults.vis.clone(),
            bounds: defaults.bounds.clone(),
        }
    }
}

macro_rules! assign_defaults {
    (cp $from: ident on $self: ident => $($prop: ident),+ $(,)?) => {
        $(
            if $self.$prop.is_none() {
                if let Some(default_val) = $from.$prop {
                    $self.$prop = Some(default_val);
                }
            }
        )+
    };
    (clone $from: ident on $self: ident => $($prop: ident),+ $(,)?) => {
        $(
            if $self.$prop.is_none() {
                if let Some(ref default_val) = $from.$prop {
                    $self.$prop = Some(default_val.clone());
                }
            }
        )+
    };
    ($from: ident on $self: ident) => {
        assign_defaults!(cp $from on $self => owned, const_fn, cp, ptr_deref);
        assign_defaults!(clone $from on $self => prefix, suffix, vis);
        $self.apply_default_bounds(&$from.bounds);
    };
}

impl VariationOptions {
    pub fn assign_defaults_from_struct(&mut self, defaults: &VariationDefaults) {
        assign_defaults!(defaults on self);
    }

    pub fn assign_defaults_from_prop_all(&mut self, defaults: &Option<Self>) {
        if let Some(defaults) = defaults {
            assign_defaults!(defaults on self);
        }
    }

    fn apply_default_bounds(&mut self, default_bounds: &Punctuated<WherePredicate, Token![,]>) {
        if self.bounds.is_empty() && !default_bounds.is_empty() {
            self.bounds.clone_from(default_bounds);
        }
    }
}

#[derive(Copy, Clone)]
#[cfg_attr(feature = "_debug", derive(Debug))]
pub enum DerefKind {
    Auto,
    Deref,
    DerefMut,
}

impl Parse for DerefKind {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            Ok(Self::Auto)
        } else if input.peek(Token![mut]) {
            input.parse::<Token![mut]>()?;
            Ok(Self::DerefMut)
        } else if input.peek(Token![ref]) {
            input.parse::<Token![ref]>()?;
            Ok(Self::Deref)
        } else {
            Err(input.error("Expected `mut`, `ref` or nothing"))
        }
    }
}

impl FromExpr for DerefKind {
    fn from_expr(expr: Expr) -> syn::Result<Self> {
        match expr {
            Expr::Reference(expr) => Ok(if expr.mutability.is_some() {
                Self::DerefMut
            } else {
                Self::Deref
            }),
            Expr::Verbatim(tokens) => syn::parse2(tokens),
            other => Err(Error::new_spanned(
                other,
                "Expected `mut`, `ref` or nothing",
            )),
        }
    }
}

impl DerefKind {
    pub fn try_into_tokens(self) -> Option<TokenStream> {
        match self {
            Self::Auto => None,
            Self::Deref => Some(Punct::new_joint('&').into_token_stream()),
            Self::DerefMut => Some(quote! { &mut }),
        }
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "_debug", derive(Debug))]
pub enum SkippableIdent {
    Ident(Ident),
    Skip,
}

impl FromExpr for SkippableIdent {
    fn from_expr(expr: Expr) -> syn::Result<Self> {
        Ok(Self::Ident(Ident::from_expr(expr)?))
    }
}

impl Parse for SkippableIdent {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(syn::Lit) {
            let lit: LitStr = input.parse()?;
            if lit.value().is_empty() {
                Ok(Self::Skip)
            } else {
                Err(Error::new_spanned(lit, "Expected empty string"))
            }
        } else {
            Ok(Self::Ident(input.parse()?))
        }
    }
}

const _: () = {
    use std::fmt::{Display, Formatter, Result, Write};
    #[derive(Copy, Clone)]
    struct Renderer<'a> {
        ident: &'a SkippableIdent,
        is_prefix: bool,
    }

    impl Display for Renderer<'_> {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            const CHAR: char = '_';

            match self.ident {
                SkippableIdent::Skip => Ok(()),
                SkippableIdent::Ident(ident) => {
                    if self.is_prefix {
                        Display::fmt(ident, f)?;
                        f.write_char(CHAR)
                    } else {
                        f.write_char(CHAR)?;
                        Display::fmt(ident, f)
                    }
                }
            }
        }
    }

    impl<'a> SkippableIdent {
        #[inline]
        pub fn as_suffix(&'a self) -> impl Display + Copy + 'a {
            Renderer {
                ident: self,
                is_prefix: false,
            }
        }

        #[inline]
        pub fn as_prefix(&'a self) -> impl Display + Copy + 'a {
            Renderer {
                ident: self,
                is_prefix: true,
            }
        }
    }
};

macro_rules! parse_opt {
    ($($for: ty),+ $(,)?) => {
        $(
          impl ParseOption for $for {
              #[inline]
              fn from_stream(input: ParseStream) -> syn::Result<Self> {
                  input.parse()
              }
          }
        )+
    };
}

parse_opt!(SkippableIdent, DerefKind);
