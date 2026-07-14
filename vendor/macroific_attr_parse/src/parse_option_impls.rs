use std::rc::Rc;
use std::sync::Arc;

use proc_macro2::{Literal, Span, TokenStream};
use quote::ToTokens;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::*;

use crate::ValueSyntax;
use crate::__attr_parse_prelude::*;

impl ParseOption for bool {
    fn from_stream(input: ParseStream) -> Result<Self> {
        input.parse_bool_attr()
    }
}

impl<T: ParseOption, P: Parse> ParseOption for Punctuated<T, P> {
    #[inline]
    fn from_stream(input: ParseStream) -> Result<Self> {
        if input.is_empty() {
            return Ok(Self::new());
        }

        let parse_buf;
        let parse_from: ParseStream;
        match ValueSyntax::from_stream(input) {
            Some(ValueSyntax::Eq) => {
                input.parse::<Token![=]>()?;
                let outer;
                parenthesized!(outer in input);

                bracketed!(parse_buf in outer);
                parse_from = &parse_buf;
            }
            Some(ValueSyntax::Paren) => {
                parenthesized!(parse_buf in input);
                parse_from = &parse_buf;
            }
            None => {
                parse_from = input;
            }
        };

        Self::parse_terminated_with(parse_from, ParseOption::from_stream)
    }
}

impl<T: FromExpr, P: Parse + Default> FromExpr for Punctuated<T, P> {
    #[cfg_attr(not(feature = "full"), inline, allow(unused_variables))]
    fn from_expr(expr: Expr) -> Result<Self> {
        #[cfg(feature = "full")]
        match expr {
            Expr::Array(ExprArray { elems, .. }) | Expr::Tuple(ExprTuple { elems, .. }) => {
                let it = elems.into_iter().map(T::from_expr);
                crate::parse_utils::try_collect(it)
            }
            expr => Err(Error::new_spanned(
                expr,
                "Can't parse this type as `Punctuated`",
            )),
        }

        #[cfg(not(feature = "full"))]
        Err(Error::new(
            Span::call_site(),
            "`full` feature required to parse `Punctuated`",
        ))
    }
}

impl<T: ParseOption> ParseOption for Option<T> {
    fn from_stream(input: ParseStream) -> Result<Self> {
        Ok(Some(T::from_stream(input)?))
    }
}

impl<T: FromExpr> FromExpr for Option<T> {
    fn from_expr(expr: Expr) -> Result<Self> {
        Ok(Some(T::from_expr(expr)?))
    }

    #[inline]
    fn boolean() -> Option<Self> {
        Some(T::boolean())
    }
}

impl FromExpr for Expr {
    #[inline]
    fn from_expr(expr: Expr) -> Result<Self> {
        Ok(expr)
    }
}

impl FromExpr for TokenStream {
    #[inline]
    fn from_expr(expr: Expr) -> Result<Self> {
        Ok(expr.into_token_stream())
    }
}

impl FromExpr for Lit {
    fn from_expr(expr: Expr) -> Result<Self> {
        Ok(ExprLit::from_expr(expr)?.lit)
    }
}

impl FromExpr for bool {
    fn from_expr(expr: Expr) -> Result<Self> {
        Ok(<LitBool>::from_expr(expr)?.value())
    }

    #[inline]
    fn boolean() -> Option<Self> {
        Some(true)
    }
}

impl FromExpr for LitBool {
    fn from_expr(expr: Expr) -> Result<Self> {
        let lit = Lit::from_expr(expr)?;
        if let Lit::Bool(lit) = lit {
            Ok(lit)
        } else {
            Err(Error::new_spanned(lit, "Incompatible literal"))
        }
    }

    fn boolean() -> Option<Self> {
        Some(Self::new(true, Span::call_site()))
    }
}

impl FromExpr for Path {
    fn from_expr(expr: Expr) -> Result<Self> {
        Ok(<ExprPath>::from_expr(expr)?.path)
    }
}

/// For use within the macro. API subject to change at any time.
macro_rules! parse_impl {
    (lit $([$base: ty, $lit: ty]),+) => {
        $(
            impl ParseOption for $base {
              fn from_stream(input: ::syn::parse::ParseStream) -> ::syn::Result<Self> {
                  Ok(<$lit as ParseOption>::from_stream(input)?.value())
                }
            }

            from_expr!(lit_direct_num_ok $base, value => $lit);
        )+
    };
    (new [$($ty: ty),+]) => {
        $(
          impl<T: ParseOption> ParseOption for $ty {
            fn from_stream(input: ParseStream) -> Result<Self> {
                T::from_stream(input).map(<$ty>::new)
            }
          }

            impl<T: FromExpr> FromExpr for $ty {
                fn from_expr(expr: Expr) -> Result<Self> {
                    T::from_expr(expr).map(<$ty>::new)
                }
            }
        )+
    };
    (parse [$($ty: ty),+]) => {
        $(
          impl ParseOption for $ty {
              parse_impl!(parse);
          }
        )+
    };
    (parse if $feature: literal [$($ty: ty),+]) => {
        $(
          #[cfg_attr(doc_cfg, doc(cfg(feature = $feature)))]
          impl ParseOption for $ty {
              parse_impl!(parse);
          }
        )+
    };
    (parse) => {
          fn from_stream(input: ::syn::parse::ParseStream) -> ::syn::Result<Self> {
            ValueSyntax::from_stream(input).and_parse(input)
          }
    };
    (lit_num [$lit: ty => $($base: ty),+]) => {
        $(
            impl ParseOption for $base {
              fn from_stream(input: ::syn::parse::ParseStream) -> ::syn::Result<Self> {
                <$lit as ParseOption>::from_stream(input)?.base10_parse()
              }
            }

            from_expr!(lit_direct_num $base, base10_parse => $lit);
        )+
    };
}

macro_rules! from_expr {
    (direct $([$ident: ident => $target: ident]),+) => {
        $(
            impl FromExpr for $target {
                fn from_expr(expr: Expr) -> Result<Self> {
                    if let Expr::$ident(expr) = expr {
                        Ok(expr)
                    } else {
                        Err(Error::new_spanned(expr, "Incompatible expression"))
                    }
                }
            }
        )+
    };
    (lit $([$ident: ident => $target: ident]),+) => {
        $(
               impl FromExpr for $target {
                    fn from_expr(expr: Expr) -> Result<Self> {
                        let lit = Lit::from_expr(expr)?;
                        if let Lit::$ident(lit) = lit {
                            Ok(lit)
                        } else {
                            Err(Error::new_spanned(lit, "Incompatible literal"))
                        }
                    }
                }
            )+
    };
    (lit_direct_num $base: ty, $fn: ident => $lit: ty) => {
        impl FromExpr for $base {
            fn from_expr(expr: Expr) -> ::syn::Result<Self> {
                <$lit>::from_expr(expr)?.$fn()
            }
        }
    };
    (lit_direct_num_ok $base: ty, $fn: ident => $lit: ty) => {
        impl FromExpr for $base {
            fn from_expr(expr: Expr) -> ::syn::Result<Self> {
                Ok(<$lit>::from_expr(expr)?.$fn())
            }
        }
    };
    (tokenise [$($ident: ident),+]) => {
        $(
            impl FromExpr for $ident {
                fn from_expr(expr: Expr) -> Result<Self> {
                    parse2(expr.into_token_stream())
                }
            }
        )+
    };
}

from_expr!(tokenise [Lifetime, LifetimeParam, BoundLifetimes, TypeParamBound, TraitBound, TypeParam, GenericParam, WherePredicate]);
from_expr!(tokenise [Ident, Type, TypeArray, TypeBareFn, TypeGroup, TypeImplTrait, TypeInfer, TypeMacro, TypeNever, TypeParen, TypePath, TypePtr, TypeReference, TypeSlice, TypeTraitObject, TypeTuple, AngleBracketedGenericArguments, ConstParam, Abi, BareFnArg, Meta, MetaList, MetaNameValue, Visibility]);
from_expr!(lit [Str => LitStr], [ByteStr => LitByteStr], [Byte => LitByte], [Char => LitChar], [Int => LitInt], [Float => LitFloat], [Verbatim => Literal]);
from_expr!(direct [Array => ExprArray], [Assign => ExprAssign], [Async => ExprAsync], [Await => ExprAwait], [Binary => ExprBinary], [Block => ExprBlock], [Break => ExprBreak], [Call => ExprCall], [Cast => ExprCast], [Closure => ExprClosure], [Const => ExprConst], [Continue => ExprContinue], [Field => ExprField], [ForLoop => ExprForLoop], [Group => ExprGroup], [If => ExprIf], [Infer => ExprInfer], [Index => ExprIndex], [Let => ExprLet], [Lit => ExprLit], [Loop => ExprLoop], [Macro => ExprMacro], [Match => ExprMatch], [MethodCall => ExprMethodCall], [Paren => ExprParen], [Path => ExprPath], [Range => ExprRange], [Reference => ExprReference], [Repeat => ExprRepeat], [Return => ExprReturn], [Struct => ExprStruct], [Try => ExprTry], [TryBlock => ExprTryBlock], [Tuple => ExprTuple], [Unary => ExprUnary], [Unsafe => ExprUnsafe], [While => ExprWhile], [Yield => ExprYield]);

parse_impl!(lit [String, LitStr], [char, LitChar]);

parse_impl!(lit_num [LitFloat => f32, f64]);
parse_impl!(lit_num [LitInt => u8, i8, u16, i16, u32, i32, u64, i64, usize, isize]);

parse_impl!(parse [Expr, AngleBracketedGenericArguments, ConstParam, Abi, BareFnArg, Ident, Path, Meta, MetaList, MetaNameValue, Visibility]);
parse_impl!(parse [Lifetime, LifetimeParam, BoundLifetimes, TypeParamBound, TraitBound, TypeParam, GenericParam, WherePredicate]);
parse_impl!(parse [Lit, LitBool, LitByteStr, LitByte, LitStr, LitChar, LitInt, LitFloat, Literal]);
parse_impl!(parse [Type, TypeArray, TypeBareFn, TypeGroup, TypeImplTrait, TypeInfer, TypeMacro, TypeNever, TypeParen, TypePath, TypePtr, TypeReference, TypeSlice, TypeTraitObject, TypeTuple]);

parse_impl!(new [Box<T>, Rc<T>, Arc<T>]);

#[cfg(feature = "full")]
parse_impl!(parse if "full" [ExprArray, ExprAssign, ExprAsync, ExprAwait, ExprBinary, ExprBlock, ExprBreak, ExprCall, ExprCast, ExprClosure, ExprConst, ExprContinue, ExprField, ExprForLoop, ExprIf, ExprIndex, ExprInfer, ExprLet, ExprLit, ExprLoop, ExprMacro, ExprMatch, ExprMethodCall, ExprParen, ExprPath, ExprRange, ExprReference, ExprRepeat, ExprReturn, ExprStruct, ExprTry, ExprTryBlock, ExprTuple, ExprUnary, ExprUnsafe, ExprWhile, ExprYield]);
