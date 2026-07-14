//! Utilities for extracting specific types of fields

use proc_macro2::Span;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Data, DataEnum, DataStruct, DataUnion, Field, Fields, FieldsNamed, FieldsUnnamed, Token,
};

use crate::seal::Sealed;

type PunctuatedFields = Punctuated<Field, Token![,]>;

/// Convert this rejection into a `syn::Error`
#[allow(missing_docs)]
pub trait ToSynError: Sealed {
    fn to_syn_err(&self) -> syn::Error;

    #[inline]
    fn into_syn_err(self) -> syn::Error
    where
        Self: Sized,
    {
        self.to_syn_err()
    }
}

/// [`Data`] extensions
pub trait DataExtractExt {
    /// Extract a union from a [`DeriveInput`](syn::DeriveInput)'s data
    ///
    /// # Errors
    /// If there's a container mismatch
    fn extract_union(self) -> Result<DataUnion, Rejection<DataStruct, DataEnum>>;

    /// Extract a struct from a [`DeriveInput`](syn::DeriveInput)'s data
    ///
    /// # Errors
    /// If there's a container mismatch
    fn extract_struct(self) -> Result<DataStruct, Rejection<DataEnum, DataUnion>>;

    /// Extract an enum from a [`DeriveInput`](syn::DeriveInput)'s data
    ///
    /// # Errors
    /// If there's a container mismatch
    fn extract_enum(self) -> Result<DataEnum, Rejection<DataStruct, DataUnion>>;

    /// Extract fields of a struct, named or unnamed
    ///
    /// # Errors
    /// If there's a container mismatch
    fn extract_struct_fields(self) -> syn::Result<PunctuatedFields>
    where
        Self: Sized,
    {
        self.extract_struct()?.fields.extract_any_fields()
    }

    /// [`extract_struct`](DataExtractExt::extract_struct) and then
    /// [`extract_named_fields`](FieldsExtractExt::extract_named_fields)
    ///
    /// # Errors
    /// If there's a container mismatch
    fn extract_struct_named(self) -> syn::Result<PunctuatedFields>
    where
        Self: Sized,
    {
        self.extract_struct()
            .map_err(syn::Error::from)?
            .fields
            .extract_named_fields()
            .map_err(syn::Error::from)
    }

    /// [`extract_struct`](DataExtractExt::extract_struct) and then
    /// [`extract_unnamed_fields`](FieldsExtractExt::extract_unnamed_fields)
    ///
    /// # Errors
    /// If there's a container mismatch
    fn extract_struct_unnamed(self) -> syn::Result<PunctuatedFields>
    where
        Self: Sized,
    {
        self.extract_struct()
            .map_err(syn::Error::from)?
            .fields
            .extract_unnamed_fields()
            .map_err(syn::Error::from)
    }
}

impl DataExtractExt for Data {
    fn extract_union(self) -> Result<DataUnion, Rejection<DataStruct, DataEnum>> {
        match self {
            Data::Struct(data) => Err(Rejection::A(data)),
            Data::Enum(data) => Err(Rejection::B(data)),
            Data::Union(data) => Ok(data),
        }
    }

    fn extract_struct(self) -> Result<DataStruct, Rejection<DataEnum, DataUnion>> {
        match self {
            Data::Struct(data) => Ok(data),
            Data::Enum(data) => Err(Rejection::A(data)),
            Data::Union(data) => Err(Rejection::B(data)),
        }
    }

    fn extract_enum(self) -> Result<DataEnum, Rejection<DataStruct, DataUnion>> {
        match self {
            Data::Struct(data) => Err(Rejection::A(data)),
            Data::Enum(data) => Ok(data),
            Data::Union(data) => Err(Rejection::B(data)),
        }
    }
}

/// [`Fields`] extensions
pub trait FieldsExtractExt {
    /// Extract named fields from [`Fields`]. `()` is returned for unit structs.
    ///
    /// # Errors
    /// If the fields are unnamed, `Err(Rejection::A)` is returned. If the fields are unit,
    /// `Err(Rejection::B)` is returned.
    fn extract_named_fields(self) -> Result<PunctuatedFields, Rejection<FieldsUnnamed, ()>>;

    /// Extract unnamed fields from [`Fields`]. `()` is returned for unit structs.
    ///
    /// # Errors
    /// If the fields are named, `Err(Rejection::A)` is returned. If the fields are unit,
    /// `Err(Rejection::B)` is returned.
    fn extract_unnamed_fields(self) -> Result<PunctuatedFields, Rejection<FieldsNamed, ()>>;

    /// Extract named or unnamed fields
    ///
    /// # Errors
    /// If it's a unit struct
    fn extract_any_fields(self) -> syn::Result<PunctuatedFields>
    where
        Self: Sized,
    {
        match self.extract_named_fields() {
            Ok(fields) => Ok(fields),
            Err(Rejection::A(fields)) => Ok(fields.unnamed),
            Err(rejection) => Err(rejection.into()),
        }
    }
}

impl FieldsExtractExt for Fields {
    /// Extract named fields from [`Fields`]. `()` is returned for unit structs.
    fn extract_named_fields(self) -> Result<PunctuatedFields, Rejection<FieldsUnnamed, ()>> {
        match self {
            Fields::Named(fields) => Ok(fields.named),
            Fields::Unnamed(fields) => Err(Rejection::A(fields)),
            Fields::Unit => Err(Rejection::B(())),
        }
    }

    /// Extract unnamed fields from [`Fields`]. `()` is returned for unit structs.
    fn extract_unnamed_fields(self) -> Result<PunctuatedFields, Rejection<FieldsNamed, ()>> {
        match self {
            Fields::Named(fields) => Err(Rejection::A(fields)),
            Fields::Unnamed(fields) => Ok(fields.unnamed),
            Fields::Unit => Err(Rejection::B(())),
        }
    }
}

/// One of two error states
#[allow(missing_docs)]
#[derive(Debug)]
pub enum Rejection<A, B> {
    A(A),
    B(B),
}

impl<A, B> From<Rejection<A, B>> for syn::Error
where
    Rejection<A, B>: ToSynError,
{
    #[inline]
    fn from(value: Rejection<A, B>) -> Self {
        value.to_syn_err()
    }
}

seal!(Rejection<FieldsUnnamed, ()>, Rejection<FieldsNamed, ()>);

impl ToSynError for Rejection<FieldsUnnamed, ()> {
    fn to_syn_err(&self) -> syn::Error {
        syn::Error::new(
            match *self {
                Self::A(ref f) => f.span(),
                Self::B(_) => Span::call_site(),
            },
            "Only named fields supported",
        )
    }
}

impl ToSynError for Rejection<FieldsNamed, ()> {
    fn to_syn_err(&self) -> syn::Error {
        syn::Error::new(
            match *self {
                Self::A(ref f) => f.span(),
                Self::B(_) => Span::call_site(),
            },
            "Only unnamed fields supported",
        )
    }
}

macro_rules! impl_reject {
    ($msg: literal => [$a: ty => $p_a: ident, $b: ty => $p_b: ident]) => {
        seal!(Rejection<$a, $b>);

        impl ToSynError for Rejection<$a, $b> {
            fn to_syn_err(&self) -> ::syn::Error {
                ::syn::Error::new(
                    match self {
                        Self::A(v) => v.$p_a.span(),
                        Self::B(v) => v.$p_b.span(),
                    },
                    $msg,
                )
            }
        }
    };
}

impl_reject!("Only structs supported" => [DataEnum => enum_token, DataUnion => union_token]);
impl_reject!("Only enums supported" => [DataStruct => struct_token, DataUnion => union_token]);
impl_reject!("Only unions supported" => [DataStruct => struct_token, DataEnum => enum_token]);
