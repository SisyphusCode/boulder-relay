//! Shared functionality for [macroific](https://docs.rs/macroific) &
//! [macroific_macro](https://docs.rs/macroific_macro).
//!
//! `macroific` is the crate you're looking for.

#![deny(clippy::correctness, clippy::suspicious)]
#![warn(clippy::complexity, clippy::perf, clippy::style, clippy::pedantic)]
#![allow(
    unknown_lints,
    clippy::module_name_repetitions,
    clippy::wildcard_imports,
    clippy::ignored_unit_patterns
)]
#![warn(missing_docs)]
#![cfg_attr(doc_cfg, feature(doc_auto_cfg))]

macro_rules! seal {
    ($($ty: ty),+) => {
        $( impl crate::seal::Sealed for $ty {} )+
    };
}

pub mod core_ext;
pub mod elements;
pub mod extract_fields;

mod seal {
    pub trait Sealed {}
}
