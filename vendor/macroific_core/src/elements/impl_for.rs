use proc_macro2::{Ident, TokenStream};
use quote::ToTokens;
use syn::{Generics, Token};

/// `impl SomeTrait for SomeType`
///
/// ```
/// # use macroific_core::{elements::*, core_ext::*};
/// # use proc_macro2::*;
/// # use syn::*;
/// # use quote::*;
///
/// let generics = Generics::default();
/// let our_trait_name: Path = parse_quote!{ ::foo::Trait };
/// let implemented_for = Ident::create("SomeStruct");
///
/// let impl_for = ImplFor::new(&generics, our_trait_name, &implemented_for);
/// assert_eq!(impl_for.into_token_stream().to_string(), "impl :: foo :: Trait for SomeStruct");
/// ```
pub struct ImplFor<'a, W> {
    generics: &'a Generics,
    what: W,
    r#for: &'a Ident,
}

impl<'a, W> ImplFor<'a, W> {
    /// `implemented_trait` - the name/path of the trait we're implementing
    /// `implemented_for` - the type we're implementing the trait for
    #[inline]
    pub const fn new(
        generics: &'a Generics,
        implemented_trait: W,
        implemented_for: &'a Ident,
    ) -> Self {
        Self {
            generics,
            what: implemented_trait,
            r#for: implemented_for,
        }
    }
}

impl<'a, W: ToTokens> ToTokens for ImplFor<'a, W> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let (g1, g2, g3) = self.generics.split_for_impl();

        <Token![impl]>::default().to_tokens(tokens);
        g1.to_tokens(tokens);
        self.what.to_tokens(tokens);
        <Token![for]>::default().to_tokens(tokens);
        self.r#for.to_tokens(tokens);
        g2.to_tokens(tokens);
        g3.to_tokens(tokens);
    }
}
