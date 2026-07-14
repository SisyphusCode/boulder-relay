//! Extension traits

use proc_macro2::{Ident, Punct, Spacing, Span};

/// [`Ident`] extensions
pub trait MacroificCoreIdentExt {
    /// Shorthand for `Ident::new(name, Span::call_site())`
    fn create(name: &str) -> Self;
}

/// [`Punct`] extensions
pub trait MacroificCorePunctExt {
    /// Create a new [`Punct`] with [`Spacing::Alone`]
    fn new_alone(ch: char) -> Self;

    /// Create a new [`Punct`] with [`Spacing::Joint`]
    fn new_joint(ch: char) -> Self;
}

impl MacroificCorePunctExt for Punct {
    #[inline]
    fn new_alone(ch: char) -> Self {
        Self::new(ch, Spacing::Alone)
    }

    #[inline]
    fn new_joint(ch: char) -> Self {
        Self::new(ch, Spacing::Joint)
    }
}

impl MacroificCoreIdentExt for Ident {
    #[inline]
    fn create(name: &str) -> Self {
        Self::new(name, Span::call_site())
    }
}
