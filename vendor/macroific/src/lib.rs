//! Proc macro development utilities
//!
//! [![MASTER CI status](https://github.com/Alorel/macroific-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/Alorel/macroific-rs/actions/workflows/ci.yml?query=branch%3Amaster)
//! [![crates.io badge](https://img.shields.io/crates/v/macroific)](https://crates.io/crates/macroific)
//! [![docs.rs badge](https://img.shields.io/docsrs/macroific?label=docs.rs)](https://docs.rs/macroific)
//! [![dependencies badge](https://img.shields.io/librariesio/release/cargo/macroific)](https://libraries.io/cargo/macroific)
//!
//! # Features
//!
//! | Feature | Description |
//! | ------- | ----------- |
//! | `default` | `["derive"]` |
//! | `attr_parse` | Attribute parsing utilities, [`attr_parse`](crate::attr_parse). |
//! | `derive` | Enable derive macros. Currently requires the `attr_parse` feature to do anything. |
//! | `full` | Enable `syn/full`. If `attr_parse` is enabled, it'll implement the traits for types that require `syn/full`. |
//! | `nightly` | Enable some nightly Rust optimisations **during macro execution only**, has no effect on generated code. |

#![deny(clippy::correctness, clippy::suspicious)]
#![warn(clippy::complexity, clippy::perf, clippy::style, clippy::pedantic)]
#![cfg_attr(doc_cfg, feature(doc_auto_cfg))]
#![warn(missing_docs)]

#[cfg(feature = "attr_parse")]
pub mod attr_parse;

pub use macroific_core::*;

#[allow(missing_docs)]
pub mod prelude {
    pub use macroific_core::core_ext::*;
    pub use macroific_core::extract_fields::{DataExtractExt, FieldsExtractExt, ToSynError};

    #[cfg(feature = "attr_parse")]
    pub use crate::attr_parse::prelude::*;
}
