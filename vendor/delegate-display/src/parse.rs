use macroific::prelude::*;
use syn::punctuated::Punctuated;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Error, Field, Fields};

use crate::ContainerOptions;
use crate::{BaseTokenStream, EnumData, FieldLike, FirstField, ParsedData};

impl ParsedData {
    pub fn parse(input: BaseTokenStream, trait_name: &str) -> syn::Result<Self> {
        let input = syn::parse::<DeriveInput>(input)?;
        Ok(Self {
            options: if input.attrs.is_empty() {
                Default::default()
            } else {
                let specific_attr = format!("dd{}", &trait_name[1..]);
                let mut both = None;
                let mut specific = None;

                for attr in input.attrs {
                    if let Some(ident) = attr.path().get_ident() {
                        macro_rules! parse {
                            ($opt: ident, $msg: expr) => {
                                if $opt.is_some() {
                                    return Err(Error::new_spanned(ident, $msg));
                                }

                                $opt = Some(attr);
                            };
                        }

                        match ident.to_string().as_str() {
                            "dboth" => {
                                parse!(both, "Cannot specify `dboth` more than once");
                            }
                            i if i == specific_attr => {
                                parse!(
                                    specific,
                                    format!("Cannot specify `{}` more than once", specific_attr)
                                );
                            }
                            _ => {}
                        }
                    }
                }

                let opts = if let Some(attr) = specific.or(both) {
                    ContainerOptions::from_attr(attr)?
                } else {
                    Default::default()
                };

                if opts.base_bounds && !opts.bounds.is_empty() {
                    return Err(Error::new_spanned(
                        opts.bounds,
                        "Cannot specify `bounds` and `base_bounds` together",
                    ));
                }

                if opts.delegate_to.is_some() && matches!(input.data, Data::Enum(_)) {
                    return Err(Error::new_spanned(
                        opts.delegate_to,
                        "Cannot specify `delegate_to` on enums",
                    ));
                }

                opts
            },
            first_field: input.data.try_into()?,
            ident: input.ident,
            generics: input.generics,
        })
    }
}

impl FirstField {
    fn load_first_field<T>(fields: Punctuated<Field, T>) -> syn::Result<Option<Box<FieldLike>>> {
        let mut fields = fields.into_iter();
        let first = match fields.next() {
            Some(f) => f,
            None => return Ok(None),
        };

        if let Some(f) = fields.next() {
            Err(Error::new_spanned(
                f,
                "The struct/enum can only have one member",
            ))
        } else {
            Ok(Some(Box::new(match first.ident {
                Some(name) => FieldLike::Ident(name, first.ty),
                None => FieldLike::Indexed(first.ty),
            })))
        }
    }
}

impl TryFrom<Data> for FirstField {
    type Error = Error;

    fn try_from(data: Data) -> Result<Self, Self::Error> {
        match data {
            Data::Enum(data) => data.try_into(),
            Data::Struct(data) => data.try_into(),
            Data::Union(s) => Err(Error::new_spanned(s.union_token, "Unions not supported")),
        }
    }
}

impl TryFrom<DataStruct> for FirstField {
    type Error = Error;

    fn try_from(value: DataStruct) -> Result<Self, Self::Error> {
        Ok(Self::Struct(match value.fields {
            Fields::Unit => None,
            Fields::Named(f) => Self::load_first_field(f.named)?,
            Fields::Unnamed(f) => Self::load_first_field(f.unnamed)?,
        }))
    }
}

impl TryFrom<DataEnum> for FirstField {
    type Error = Error;

    fn try_from(value: DataEnum) -> Result<Self, Self::Error> {
        let it = value
            .variants
            .into_iter()
            .map(move |var| -> syn::Result<EnumData> {
                let first_field = match var.fields {
                    Fields::Unit => None,
                    Fields::Unnamed(f) => Self::load_first_field(f.unnamed)?,
                    Fields::Named(f) => Self::load_first_field(f.named)?,
                };
                Ok((var.ident, first_field))
            });

        Ok(Self::Enum(macroific::attr_parse::__private::try_collect(
            it,
        )?))
    }
}
