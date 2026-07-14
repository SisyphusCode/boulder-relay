<!-- cargo-rdme start -->

Proc macro development utilities

[![MASTER CI status](https://github.com/Alorel/macroific-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/Alorel/macroific-rs/actions/workflows/ci.yml?query=branch%3Amaster)
[![crates.io badge](https://img.shields.io/crates/v/macroific)](https://crates.io/crates/macroific)
[![docs.rs badge](https://img.shields.io/docsrs/macroific?label=docs.rs)](https://docs.rs/macroific)
[![dependencies badge](https://img.shields.io/librariesio/release/cargo/macroific)](https://libraries.io/cargo/macroific)

# Features

| Feature | Description |
| ------- | ----------- |
| `default` | `["derive"]` |
| `attr_parse` | Attribute parsing utilities, [`attr_parse`](https://docs.rs/macroific/latest/macroific/attr_parse/). |
| `derive` | Enable derive macros. Currently requires the `attr_parse` feature to do anything. |
| `full` | Enable `syn/full`. If `attr_parse` is enabled, it'll implement the traits for types that require `syn/full`. |
| `nightly` | Enable some nightly Rust optimisations **during macro execution only**, has no effect on generated code. |

<!-- cargo-rdme end -->
