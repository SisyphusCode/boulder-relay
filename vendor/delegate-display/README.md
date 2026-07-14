<!-- cargo-rdme start -->

Lets you derive `Display` & `Debug` traits on structs with
`0..=1` fields & enums where each variant has `0..=1` fields - see input/output examples below.

[![master CI badge](https://img.shields.io/github/actions/workflow/status/Alorel/delegate-display-rs/ci.yml?label=master%20CI)](https://github.com/Alorel/delegate-display-rs/actions/workflows/ci.yml?query=branch%3Amaster)
[![crates.io badge](https://img.shields.io/crates/v/delegate-display)](https://crates.io/crates/delegate-display)
[![docs.rs badge](https://img.shields.io/docsrs/delegate-display?label=docs.rs)](https://docs.rs/delegate-display)
[![dependencies badge](https://img.shields.io/librariesio/release/cargo/delegate-display)](https://libraries.io/cargo/delegate-display)

# Examples

<details><summary>Newtype structs</summary>

```rust
// Input
#[derive(delegate_display::DelegateDisplay)]
struct Foo(SomeType);

// Output
impl fmt::Display for Foo {
  #[inline]
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Display::fmt(&self.0, f)
  }
}
````

</details>

<details><summary>Structs with one field</summary>

```rust
// Input
#[derive(delegate_display::DelegateDebug)]
struct Foo { some_field: SomeType }

// Output
impl fmt::Debug for Foo {
  #[inline]
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Debug::fmt(&self.some_field, f)
  }
}
````

</details>

<details><summary>Enums</summary>

```rust
// Input
enum MyEnum {
  Foo,
  Bar(SomeType),
  Qux { baz: SomeType }
}

// Output
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
  match self {
    Self::Foo => f.write_str("Foo"),
    Self::Bar(inner) => DebugOrDisplay::fmt(inner, f),
    Self::Qux { baz } => DebugOrDisplay::fmt(baz, f),
  }
}
````

</details>

<details><summary>Empty structs & enums</summary>

```rust
// Input
struct Foo;
struct Bar{}
struct Qux();
enum Baz {}

// Output
fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
  Ok(())
}
````

</details>

<details><summary>Custom generic bounds</summary>

The attribute names are `ddebug` for `Debug`, `ddisplay` for `Display` and `dboth` for a common config for
both. `ddebug` and `ddisplay` take precendence over `dboth`.

- `base_bounds` will add whatever trait is being derived as a generic bound to each of the struct/enum's generic params
- `bounds(...)` will let you specify specific bounds

```rust
// Input
#[derive(DelegateDisplay, DelegateDebug)]
#[dboth(base_bounds)]
#[ddisplay(bounds(F: Display, B: Clone + Display))]
enum Foo<F, B> {
  Foo(F),
  Bar(B),
}

// Output
impl<F: Display, B: Clone + Display> Display for Foo<F, B> { /* ... */}
impl<F: Debug, B: Debug> Debug for Foo<F, B> { /* ... */ }
````

</details>

<details><summary>Typed delegations</summary>

Can be useful for further prettifying the output.

```rust
/// Some type that `Deref`s to the type we want to use in our formatting, in this case, `str`.
#[derive(Debug)]
struct Wrapper(&'static str);

#[derive(DelegateDebug)]
#[ddebug(delegate_to(str))] // ignore `Wrapper` and debug the `str` it `Deref`s instead
struct Typed(Wrapper);

#[derive(DelegateDebug)] // Included for comparison
struct Base(Wrapper);

assert_eq!(format!("{:?}", Typed(Wrapper("foo"))), "\"foo\"");
assert_eq!(format!("{:?}", Base(Wrapper("bar"))), "Wrapper(\"bar\")");
```

</details>

<details><summary>Invalid inputs</summary>

```rust
#[derive(DelegateDisplay, Debug)]
#[dboth(delegate_to(String))] // `delegate_to` is not supported on enums
enum SomeEnum {
  Foo(Arc<String>)
}
```

```rust
#[derive(delegate_display::DelegateDisplay)]
#[ddisplay(base_bounds, bounds(T: Display))] // `base_bounds` and `bounds` are mutually exclusive
struct Generic<T>(T);
```

```rust
#[derive(delegate_display::DelegateDisplay)]
#[ddisplay(base_bounds)]
#[ddisplay(base_bounds)] // `dbodh` and `ddisplay` can be mixed, but the same option can't be used twice
struct Foo<T>(T);
```

```rust
#[derive(delegate_display::DelegateDebug)]
struct TooManyFields1 {
  foo: u8,
  bar: u8, // Only one field permitted
}
```

```rust
#[derive(delegate_display::DelegateDebug)]
struct TooManyFields2(u8, u8); // too many fields
```

```rust
#[derive(delegate_display::DelegateDebug)]
enum SomeEnum {
  A, // this is ok
  B(u8), // this is ok
  C { foo: u8 }, // this is ok
  D(u8, u8), // Only one field permitted
  E { foo: u8, bar: u8 } // Only one field permitted
}
```

```rust
#[derive(delegate_display::DelegateDebug)]
union Foo { bar: u8 } // Unions are not supported
```

</details>

<!-- cargo-rdme end -->
