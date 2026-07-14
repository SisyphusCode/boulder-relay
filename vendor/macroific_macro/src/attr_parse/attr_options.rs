use syn::Token;

use macroific_core::core_ext::*;

use super::PRIVATE;

common_impl!(AttrOptionsDerive "AttributeOptions");

pub struct AttrOptionsDerive {
    ident: Ident,
    generics: Generics,
    fields: Fields,
}

impl Parse for AttrOptionsDerive {
    fn parse(input: ParseStream) -> ::syn::Result<Self> {
        let (ident, generics, fields, _) = super::common_construct(input)?;

        Ok(Self {
            ident,
            generics,
            fields,
        })
    }
}

impl ToTokens for AttrOptionsDerive {
    common_impl!(to_tokens);

    fn to_token_stream(&self) -> TokenStream {
        let fields = match self.named_fields() {
            Ok(fields) => fields,
            Err(delim) => return self.render_empty(delim),
        };

        let mut tokens = self.impl_for();

        let span_arg_name = if fields.iter().any(move |f| f.opts.omit_default()) {
            Ident::create("attributes_span")
        } else {
            Ident::create("_")
        };

        let fn_body = Group::new(Delimiter::Brace, {
            let indexed_fields = super::indexed_fields(fields);
            let nones = super::nones(fields);

            let matches = indexed_fields.clone()
                .map(move |(option_var_name, field)| {
                    let mut stream = field.resolved_label().into_token_stream();
                    <Token![=>]>::default().to_tokens(&mut stream);

                    stream.append(Group::new(
                        Delimiter::Brace,
                        quote! { #PRIVATE::decode_attr_options_field(&mut #option_var_name, ident, meta.input) },
                    ));

                    stream
                });

            let unwraps = super::unwraps(indexed_fields, &span_arg_name);

            quote! {
                #nones

                for attr in attributes {
                    attr.parse_nested_meta(|meta| {
                        let ident = #PRIVATE::get_attr_ident(&meta.path)?;

                        match ::std::string::ToString::to_string(ident).as_str() {
                            #(#matches)*
                            other => #RESULT::Err(::syn::Error::new(::syn::spanned::Spanned::span(ident), ::std::format!("Unrecognised attribute: `{}`", other))),
                        }
                    })?;
                }

                #RESULT::Ok(Self #unwraps )
            }
        });

        // Struct body
        tokens.append(Group::new(Delimiter::Brace, {
            let mut signature = quote! {
                fn from_iter(#span_arg_name: ::proc_macro2::Span, attributes: impl ::core::iter::IntoIterator<Item = ::syn::Attribute>) -> ::syn::Result<Self>
            };
            signature.append(fn_body);
            signature
        }));

        tokens
    }
}

impl AttrOptionsDerive {
    #[allow(clippy::needless_pass_by_value)]
    fn render_empty_body(ending: Option<Group>) -> TokenStream {
        quote! {
            #INLINE
            fn from_attr(_: ::syn::Attribute) -> ::syn::Result<Self> {
                #RESULT::Ok(Self #ending)
            }

            #INLINE
            fn from_iter(_: ::proc_macro2::Span, _: impl ::core::iter::IntoIterator<Item = ::syn::Attribute>) -> ::syn::Result<Self> {
                #RESULT::Ok(Self #ending)
            }
        }
    }
}
