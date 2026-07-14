use macroific::prelude::*;
use proc_macro2::Ident;
use syn::punctuated::Punctuated;
use syn::{Token, Visibility, WherePredicate};

use super::options::{DerefKind, SkippableIdent};
use super::{VariationDefaults, VariationOptions};

#[cfg_attr(feature = "_debug", derive(Debug))]
pub struct FinalOptions {
    pub owned: bool,
    pub const_fn: bool,
    pub skip: bool,
    pub cp: bool,
    pub ptr_deref: Option<DerefKind>,
    pub vis: Visibility,
    pub prefix: Option<SkippableIdent>,
    pub suffix: Option<SkippableIdent>,
    pub ty: Option<syn::Type>,
    pub bounds: Punctuated<WherePredicate, Token![,]>,
}

#[cfg_attr(feature = "_debug", derive(Debug))]
pub struct Naming {
    pub prefix: Option<&'static str>,
    pub suffix: Option<&'static str>,
}

impl Naming {
    pub const GET: Self = Self {
        prefix: None,
        suffix: None,
    };
    pub const GET_MUT: Self = Self {
        prefix: None,
        suffix: Some("mut"),
    };
    pub const SET: Self = Self {
        prefix: Some("set"),
        suffix: None,
    };
}

impl FinalOptions {
    pub fn new(
        enable_by_default: bool,
        defaults_from_struct: &VariationDefaults,
        defaults_for_variation: &'static Naming,
        opts: Option<VariationOptions>,
        opts_all_field: &Option<VariationOptions>,
        opts_all_container: &VariationDefaults,
    ) -> Option<Self> {
        let mut opts = match (enable_by_default, opts) {
            (
                _,
                Some(VariationOptions {
                    skip: Some(skip), ..
                }),
            ) if skip => return None,
            (_, Some(mut opts)) => {
                opts.assign_defaults_from_prop_all(opts_all_field);
                opts.assign_defaults_from_struct(defaults_from_struct);
                opts
            }
            (true, None) => {
                if let Some(opts_all_field) = opts_all_field {
                    let mut opts_all_field = opts_all_field.clone();
                    opts_all_field.assign_defaults_from_struct(defaults_from_struct);
                    opts_all_field
                } else {
                    defaults_from_struct.into()
                }
            }
            _ => return None,
        };
        opts.assign_defaults_from_struct(opts_all_container);

        let mut out = Self {
            owned: opts.owned.unwrap_or_default(),
            const_fn: opts.const_fn.unwrap_or_default(),
            skip: opts.skip.unwrap_or_default(),
            cp: opts.cp.unwrap_or_default(),
            ptr_deref: opts.ptr_deref,
            vis: opts
                .vis
                .unwrap_or_else(move || Visibility::Public(Default::default())),
            prefix: opts.prefix,
            suffix: opts.suffix,
            ty: opts.ty,
            bounds: opts.bounds,
        };
        out.apply_naming_defaults(defaults_for_variation);

        Some(out)
    }

    pub fn apply_naming_defaults(&mut self, defaults: &'static Naming) {
        if self.prefix.is_none() {
            if let Some(v) = defaults.prefix {
                self.prefix = Some(SkippableIdent::Ident(Ident::create(v)));
            }
        }
        if self.suffix.is_none() {
            if let Some(v) = defaults.suffix {
                self.suffix = Some(SkippableIdent::Ident(Ident::create(v)));
            }
        }
    }
}
