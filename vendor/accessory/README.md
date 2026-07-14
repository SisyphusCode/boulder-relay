<!-- cargo-rdme start -->

A nice and configurable derive macro for getters & setters. See derive macro
docs for full list of options

[![MASTER CI status](https://github.com/Alorel/accessory-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/Alorel/accessory-rs/actions/workflows/ci.yml?query=branch%3Amaster)
[![crates.io badge](https://img.shields.io/crates/v/accessory)](https://crates.io/crates/accessory)
[![docs.rs badge](https://img.shields.io/docsrs/accessory?label=docs.rs)](https://docs.rs/accessory)
[![dependencies badge](https://img.shields.io/librariesio/release/cargo/accessory)](https://libraries.io/cargo/accessory)

# Examples

<details><summary>Basic usage</summary>

```rust
#[derive(Default, accessory::Accessors)]
struct Structopher {
  /// The comment gets copied over
  #[access(set, get, get_mut)] // Generate a setter, getter ant mut getter
  field: String,
  _field2: u8, // Generate nothing
}
let mut data = Structopher::default();
data.set_field("Hello, world!".to_string());

let get: &String = data.field();
assert_eq!(get, "Hello, world!", "get(1)");

let mut get: &mut String = data.field_mut();
*get = "Hello, universe!".to_string();

let mut get = data.field();
assert_eq!(get, "Hello, universe!", "get(2)");
```

### Generated output

```rust
impl Structopher {
    /// The comment gets copied over
    #[inline]
    pub fn field(&self) -> &String { &self.field }

    /// The comment gets copied over
    #[inline]
    pub fn field_mut(&mut self) -> &mut String { &mut self.field }

    /// The comment gets copied over
    #[inline]
    pub fn set_field(&mut self, new_value: String) -> &mut Self {
        self.field = new_value;
        self
    }
}
````

</details>

<details><summary>Option inheritance</summary>

Option priority is as follows:

1. Field attribute
   1. Per-accessor type (`get`, `get_mut`, `set`)
   1. Catch-all (`all`)
1. Container attribute (`defaults`)
   1. Per-accessor type (`get`, `get_mut`, `set`)
   1. Catch-all (`all`)

```rust
#[derive(accessory::Accessors, Default, Eq, PartialEq, Debug)]
#[access(
  get, set, // derive these for all fields by default
  // set defaults for whenever
  defaults(
    all(
      const_fn, // Make it a const fn
      owned, // use `self` and not `&self`
      cp // Treat it as a copy type. Treats it as a reference if not set & not `owned`
    ),
    get(
      owned = false, // overwrite from `all`
      vis(pub(crate)) // set visibilty to `pub(crate)`
    )
  )
)]
struct Structopher {
    #[access(
      all(const_fn = false), // Disable the container's const_fn for this field
      get(const_fn),  // But re-enable it for the getter
      get_mut // enable with defaults
    )]
    x: i8,
    y: i8,

    #[access(get_mut(skip))] // skip only get_mut
    z: i8,

    #[access(skip)] // skip this field altogether
    w: i8,
}

const INST: Structopher = Structopher { x: 0, y: 0, z: 0, w: 0 }
  .set_y(-10)
  .set_z(10);

let mut inst = Structopher::default();
inst = inst.set_x(10);
*inst.x_mut() += 1;

assert_eq!(INST, Structopher { x: 0, y: -10, z: 10, w: 0 } , "const instance");
assert_eq!(inst, Structopher { x: 11, y: 0, z: 0, w: 0 } , "instance");
```

### Generated output

```rust
impl Structopher {
    #[inline]
    pub(crate) const fn x(&self) -> i8 { self.x }

    #[inline]
    pub fn x_mut(mut self) -> i8 { self.x }

    #[inline]
    pub fn set_x(mut self, new_value: i8) -> Self {
        self.x = new_value;
        self
    }

    #[inline]
    pub(crate) const fn y(&self) -> i8 { self.y }

    #[inline]
    pub const fn set_y(mut self, new_value: i8) -> Self {
        self.y = new_value;
        self
    }

    #[inline]
    pub(crate) const fn z(&self) -> i8 { self.z }

    #[inline]
    pub const fn set_z(mut self, new_value: i8) -> Self {
        self.z = new_value;
        self
    }
}
````

</details>

<details><summary>Names & types</summary>

You can modify function return types & names

```rust
#[derive(Default, accessory::Accessors)]
#[access(defaults(get(prefix(get))))]
struct Structopher {
    #[access(
      get(suffix(right_now), ty(&str)), // set the suffix and type
      get_mut(suffix("")) // remove the inherited suffix set by `get_mut`
    )]
    good: String,
}
let mut inst = Structopher::default();
*inst.good() = "On it, chief".into();
assert_eq!(inst.get_good_right_now(), "On it, chief");
```

### Generated output

```rust
impl Structopher {
    #[inline]
    pub fn get_good_right_now(&self) -> &str { &self.good }

    #[inline]
    pub fn good(&mut self) -> &mut String { &mut self.good }
}
````

</details>

<details><summary>Generic bounds</summary>

```rust
#[derive(Default, accessory::Accessors)]
#[access(bounds(World: PartialEq))] // applies to the impl block
struct Hello<World> {
  #[access(get(cp, bounds(World: Copy)))] // Applies to specific accessor
  world: World,
}

let world: u8 = Hello { world: 10u8 }.world();
assert_eq!(world, 10);
```

### Generated output

```rust
impl<World> Hello<World> where World: PartialEq {
  #[inline]
  pub fn world(&self) -> World where World: Copy {
    self.world
  }
}
````

</details>

<details><summary>Dereferencing raw pointers</summary>

The library supports dereferencing raw pointers, making them invisible to outside code. Let's have a look at our
sample struct and then we'll break it down field by field.

```rust
#[derive(Accessors)]
#[access(get, get_mut, set, defaults(all(ptr_deref())))]
struct NotUnsafeWhatsoever {
    direct: *mut String,
    
    #[access(get(ty(&str)), get_mut(ty(&mut str)), set(skip))]
    retyped: *mut String,
    
    #[access(get(ptr_deref(mut)), get_mut(skip), set(skip))]
    force_mutable: *mut NoImmutablesHere,
    
    #[access(get(cp), get_mut(cp))]
    copy_field: *mut usize,
}

// Setting up
let mut direct = String::from("direct");
let mut retyped = String::from("retyped");
let mut force_mutable = NoImmutablesHere::default();
let mut copy_field = 100;

let mut inst = NotUnsafeWhatsoever {
    direct: &mut direct,
    retyped: &mut retyped,
    force_mutable: &mut force_mutable,
    copy_field: &mut copy_field,
};

// Check `direct`
inst.direct_mut().push_str("ly opposed to this");
assert_eq!(&*direct, "directly opposed to this");
assert_eq!(inst.direct(), &*direct);

inst.set_direct(String::from("too big for the two of us"));
assert_eq!(&*direct, "too big for the two of us");
assert_eq!(inst.direct(), &*direct);


// Check `retyped`
assert_eq!(inst.retyped(), "retyped");
let (re, _) = inst.retyped_mut().split_at_mut(2);
re.make_ascii_uppercase();
assert_eq!(inst.retyped(), "REtyped");


// Check `force_mutable` - just a type check
let _fmut: &mut NoImmutablesHere = inst.force_mutable();


// Check `copy_field`
*inst.copy_field_mut() += 1;
assert_eq!(inst.copy_field(), 101);
assert_eq!(copy_field, 101);

inst.set_copy_field(777);
assert_eq!(inst.copy_field(), 777);
assert_eq!(copy_field, 777);
```

The `direct` field inherited the default auto `ptr_deref()` and resulted in the following code getting generated:
with no type modification

```rust
    #[inline]
    pub fn direct(&self) -> &String {
        unsafe { &*self.direct }
    }

    #[inline]
    pub fn direct_mut(&mut self) -> &mut String {
        unsafe { &mut *self.direct }
    }

    #[inline]
    pub fn set_direct(&mut self, new_value: String) -> &mut Self {
        unsafe {
            *self.direct = new_value;
        };
        self
    }
````

The `retyped` field has its type explicitly set on `get` and `get_mut` which got propagated to the dereference:

```rust
   #[inline]
    pub fn retyped(&self) -> &str {
        unsafe { &*self.retyped }
    }

    #[inline]
    pub fn retyped_mut(&mut self) -> &mut str {
        unsafe { &mut *self.retyped }
    }
````

`force_mutable` assumes we're dealing with some internal code and hacking our way around Rust's compile-time borrow
checks and lets us dereference a mutable reference to `NoImmutablesHere` from an immutable reference to
`NotUnsafeWhatsoever`:

```rust
   #[inline]
    pub fn force_mutable(&self) -> &mut NoImmutablesHere {
        unsafe { &mut *self.force_mutable }
    }
````

Finally, `copy` field is marked with `cp` and will not be returning a reference with `get`:

```rust
    #[inline]
    pub fn copy_field(&self) -> usize {
        unsafe { *self.copy_field }
    }

    #[inline]
    pub fn copy_field_mut(&mut self) -> &mut usize {
        unsafe { &mut *self.copy_field }
    }

    #[inline]
    pub fn set_copy_field(&mut self, new_value: usize) -> &mut Self {
        unsafe {
            *self.copy_field = new_value;
        };
        self
    }
````


</details>

<!-- cargo-rdme end -->
