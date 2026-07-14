use proc_macro2::Span;
use syn::{Attribute, Token};

use macroific_attr_parse::AttributeOptions;
use macroific_attr_parse::__private::decode_attr_options_field;
use macroific_core::core_ext::MacroificCoreIdentExt;
use macroific_core::elements::{ImplFor, ModulePrefix};

use super::{ATTR_NAME, BASE, PRIVATE};

struct Options {
    from_parse: bool,
}

impl AttributeOptions for Options {
    fn from_iter(_: Span, attributes: impl IntoIterator<Item = Attribute>) -> syn::Result<Self> {
        let mut from_parse = None;

        for attr in attributes {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("from_parse") {
                    decode_attr_options_field(&mut from_parse, &meta.path, meta.input)
                } else {
                    Ok(())
                }
            })?;
        }

        Ok(Self {
            from_parse: from_parse.unwrap_or(false),
        })
    }
}

common_impl!(ParseOptionDerive "ParseOption");

pub struct ParseOptionDerive {
    ident: Ident,
    generics: Generics,
    fields: Fields,
    opts: Options,
}

impl Parse for ParseOptionDerive {
    fn parse(input: ParseStream) -> ::syn::Result<Self> {
        let (ident, generics, fields, attrs) = super::common_construct(input)?;

        Ok(Self {
            opts: Options::from_iter_named(ATTR_NAME, Span::call_site(), attrs)?,
            ident,
            generics,
            fields,
        })
    }
}

impl ParseOptionDerive {
    #[inline]
    fn to_tokens_from_parse(&self) -> TokenStream {
        let mut tokens = self.impl_for();

        // Impl body
        tokens.append(Group::new(
            Delimiter::Brace,
            quote! {
                #INLINE
                fn from_stream(stream: ::syn::parse::ParseStream) -> ::syn::Result<Self> {
                    #PRIVATE::decode_parse_option_from_parse(stream)
                }
            },
        ));

        tokens
    }

    #[inline]
    fn to_tokens_base(&self) -> TokenStream {
        let fields = match self.named_fields() {
            Ok(fields) => fields,
            Err(delim) => return self.render_empty(delim),
        };

        let mut tokens = self.impl_for();

        let fn_body = Group::new(Delimiter::Brace, {
            let indexed_fields = super::indexed_fields(fields);

            let matches = indexed_fields.clone()
                .map(move |(option_var_name, field)| {
                    let mut stream = field.resolved_label().into_token_stream();
                    <Token![=>]>::default().to_tokens(&mut stream);

                    stream.append(Group::new(
                        Delimiter::Brace,
                        quote! { #PRIVATE::decode_parse_option_field(&mut #option_var_name, ident, value_source) },
                    ));

                    stream
                });

            let mut out = super::nones(fields);

            out.append_all(quote! {
                // Provided ident, but no value, then continued to provide the next ident
                if !parse.peek(::syn::Token![,]) {
                    for result in #PRIVATE::iterate_option_meta(parse)? {
                        let (ident, value_source) = result?;

                        match ::std::string::ToString::to_string(&ident).as_str() {
                            #(#matches)*
                            other => return #RESULT::Err(::syn::Error::new(::syn::spanned::Spanned::span(&ident), ::std::format!("Unrecognised attribute: `{}`", other))),
                        }?;

                    }
                }
            });

            RESULT.with_ident(Ident::create("Ok")).to_tokens(&mut out);

            out.append(Group::new(Delimiter::Parenthesis, {
                let mut out = <Token![Self]>::default().into_token_stream();
                let unwraps = super::unwraps(
                    indexed_fields,
                    &quote! {
                        ::proc_macro2::Span::call_site()
                    },
                );
                out.append(unwraps);
                out
            }));

            out
        });

        // Impl body
        tokens.append(Group::new(Delimiter::Brace, {
            let mut signature = quote! {
                fn from_stream(parse: ::syn::parse::ParseStream) -> ::syn::Result<Self>
            };
            signature.append(fn_body);
            signature
        }));

        ImplFor::new(
            self.generics(),
            ModulePrefix::new(&["syn", "parse", "Parse"]),
            self.ident(),
        )
        .to_tokens(&mut tokens);

        tokens.append(Group::new(
            Delimiter::Brace,
            quote! {
                #INLINE
                fn parse(parse: ::syn::parse::ParseStream) -> ::syn::Result<Self> {
                    #BASE::ParseOption::from_stream(parse)
                }
            },
        ));

        tokens
    }
}

impl ToTokens for ParseOptionDerive {
    common_impl!(to_tokens);

    fn to_token_stream(&self) -> TokenStream {
        if self.opts.from_parse {
            self.to_tokens_from_parse()
        } else {
            self.to_tokens_base()
        }
    }
}

impl ParseOptionDerive {
    #[allow(clippy::needless_pass_by_value)]
    fn render_empty_body(ending: Option<Group>) -> TokenStream {
        quote! {
            #INLINE
            fn from_stream(_: ::syn::parse::ParseStream) -> ::syn::Result<Self> {
                #RESULT::Ok(Self #ending)
            }
        }
    }
}
