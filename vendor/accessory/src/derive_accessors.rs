use macroific::elements::SimpleAttr;
use macroific::prelude::*;
use proc_macro2::{Delimiter, Group, Ident, Punct, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Attribute, DeriveInput, Generics, Token, Type};

use options::*;
use parsed_field::*;

use crate::derive_accessors::final_options::{FinalOptions, Naming};

mod final_options;
pub mod options;
mod parsed_field;

const ATTR_NAME: &str = "access";

pub struct DeriveAccessors {
    fields: Vec<ParsedField>,
    container_opts: ContainerOptions,
    ident: Ident,
    generics: Generics,
}

impl ToTokens for DeriveAccessors {
    fn to_tokens(&self, _: &mut TokenStream) {
        unimplemented!()
    }

    fn into_token_stream(self) -> TokenStream
    where
        Self: Sized,
    {
        macro_rules! variations {
            (
                [
                    $final_opts: ident,
                    $container_opts: ident,
                    $naming: ident,
                    $opts: ident,
                    $tokens: ident,
                    $ident: ident,
                    $comments: ident,
                    $ty: ident
                ] => $([$lower: ident $upper: ident $render: ident]),+
                | [$last_lower: ident $last_upper: ident $last_render: ident]
            ) => {
                $(
                    if let Some(opts) = $final_opts::new(
                        $container_opts.$lower,
                        &$container_opts.defaults.$lower,
                        &$naming::$upper,
                        $opts.$lower,
                        &$opts.all,
                        &$container_opts.defaults.all,
                    ) {
                        render_common(&mut $tokens, &$ident, &$comments, &opts, Some(SimpleAttr::new_outer("must_use")));
                        $tokens.extend($render(&$ident, &$ty, opts));
                    }
                )+

                if let Some(opts) = $final_opts::new(
                    $container_opts.$last_lower,
                    &$container_opts.defaults.$last_lower,
                    &$naming::$last_upper,
                    $opts.$last_lower,
                    &$opts.all,
                    &$container_opts.defaults.all,
                ) {
                    render_common(&mut $tokens, &$ident, &$comments, &opts, None);
                    $tokens.extend($last_render($ident, $ty, opts));
                }
            };
        }

        let Self {
            fields,
            container_opts,
            ident,
            mut generics,
        } = self;

        let mut out = {
            if !container_opts.bounds.is_empty() {
                generics
                    .make_where_clause()
                    .predicates
                    .extend(container_opts.bounds);
            }

            let (g1, g2, g3) = generics.split_for_impl();
            quote! {
                #[automatically_derived]
                impl #g1 #ident #g2 #g3
            }
        };
        drop(generics);

        out.append(Group::new(Delimiter::Brace, {
            let mut tokens = TokenStream::new();

            for ParsedField {
                comments,
                opts,
                ident,
                ty,
            } in fields
            {
                if opts.skip {
                    continue;
                }
                variations!(
                    [FinalOptions, container_opts, Naming, opts, tokens, ident, comments, ty] =>
                    [get GET RENDER_GET],
                    [get_mut GET_MUT RENDER_GET_MUT]
                    | [set SET RENDER_SET]
                );
            }

            tokens
        }));

        out
    }
}

fn render_common(
    tokens: &mut TokenStream,
    ident: &Ident,
    comments: &[Attribute],
    opts: &FinalOptions,
    must_use: Option<SimpleAttr>,
) {
    tokens.append_all(comments);
    SimpleAttr::INLINE.to_tokens(tokens);
    must_use.to_tokens(tokens);
    opts.vis.to_tokens(tokens);

    if opts.const_fn {
        tokens.append(Ident::create("const"));
    }

    tokens.append(Ident::create("fn"));

    tokens.append(match (&opts.prefix, &opts.suffix) {
        (Some(p), Some(s)) => Ident::create(&format!("{}{ident}{}", p.as_prefix(), s.as_suffix())),
        (Some(p), None) => Ident::create(&format!("{}{ident}", p.as_prefix())),
        (None, Some(s)) => Ident::create(&format!("{ident}{}", s.as_suffix())),
        (None, None) => ident.clone(),
    });
}

fn arg_ref(owned: bool) -> Option<Token![&]> {
    if owned {
        None
    } else {
        Some(<Token![&]>::default())
    }
}

fn mk_where<T, P>(bounds: Punctuated<T, P>) -> Option<TokenStream>
where
    T: ToTokens,
    P: ToTokens,
{
    if bounds.is_empty() {
        None
    } else {
        let bounds = bounds.into_token_stream();
        let mut out = TokenStream::new();
        out.append(Ident::create("where"));
        out.extend(bounds);
        Some(out)
    }
}

fn resolve_ptr_ty(base: &Type, opt: Option<DerefKind>) -> &Type {
    match (opt, base) {
        (Some(_), Type::Ptr(ptr)) => &ptr.elem,
        (_, ty) => ty,
    }
}

type RenderFieldFn = fn(&Ident, &Type, FinalOptions) -> TokenStream;
type LastRenderFieldFn = fn(Ident, Type, FinalOptions) -> TokenStream;

const RENDER_GET: RenderFieldFn = |ident, ty, opts| {
    let arg_ref = arg_ref(opts.owned);

    let val_ref = if opts.cp || opts.owned {
        None
    } else if let Some(deref) = opts.ptr_deref {
        Some(
            deref
                .try_into_tokens()
                .unwrap_or_else(move || Punct::new_joint('&').into_token_stream()),
        )
    } else {
        Some(Punct::new_joint('&').into_token_stream())
    };

    let fn_return = if let Some(ty) = opts.ty {
        ty.into_token_stream()
    } else {
        let ty = resolve_ptr_ty(ty, opts.ptr_deref);

        quote! { #val_ref #ty }
    };

    let where_clause = mk_where(opts.bounds);

    let mut out = quote! { (#arg_ref self) -> #fn_return #where_clause };
    out.append(Group::new(
        Delimiter::Brace,
        if opts.ptr_deref.is_some() {
            quote! { unsafe { #val_ref *self.#ident } }
        } else {
            quote! { #val_ref self.#ident }
        },
    ));

    out
};
const RENDER_GET_MUT: RenderFieldFn = |ident, ty, opts| {
    let fn_return = if let Some(ty) = opts.ty {
        ty.into_token_stream()
    } else {
        let ty = resolve_ptr_ty(ty, opts.ptr_deref);

        quote! { &mut #ty }
    };

    let where_clause = mk_where(opts.bounds);
    let mut out = quote! { (&mut self) -> #fn_return #where_clause };
    out.append(Group::new(
        Delimiter::Brace,
        if opts.ptr_deref.is_some() {
            quote! { unsafe { &mut *self.#ident } }
        } else {
            quote! { &mut self.#ident }
        },
    ));

    out
};

const RENDER_SET: LastRenderFieldFn = |ident, ty, opts| {
    let arg_ref = arg_ref(opts.owned);

    let self_ref = if opts.cp || opts.owned {
        None
    } else {
        Some(quote! { &mut })
    };

    let arg_ty = if let Some(ref ty) = opts.ty {
        ty
    } else {
        resolve_ptr_ty(&ty, opts.ptr_deref)
    };

    let where_clause = mk_where(opts.bounds);
    let assignment = if opts.ptr_deref.is_some() {
        quote! { unsafe { *self.#ident = new_value; } }
    } else {
        quote! { self.#ident = new_value }
    };

    quote! {
        (#arg_ref mut self, new_value: #arg_ty) -> #self_ref Self #where_clause {
            #assignment;
            self
        }
    }
};

impl Parse for DeriveAccessors {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let DeriveInput {
            attrs,
            ident,
            generics,
            data,
            ..
        } = input.parse()?;

        let container_opts = ContainerOptions::from_iter_named(ATTR_NAME, ident.span(), attrs)?;
        let fields = data
            .extract_struct_named()?
            .into_iter()
            .map(ParsedField::try_from);

        Ok(Self {
            fields: macroific::attr_parse::__private::try_collect(fields)?,
            container_opts,
            ident,
            generics,
        })
    }
}
