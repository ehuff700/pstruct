# pstruct

[![Crates.io](https://img.shields.io/crates/v/pstruct)](https://crates.io/crates/pstruct)
[![Documentation](https://docs.rs/pstruct/badge.svg)](https://docs.rs/pstruct)

A Rust procedural macro for generating pointer struct implementations with field offset access. The purpose of this crate is to minimize stack space, preventing struct copies<sup>1</sup>, while still allowing ergonomic field access. This macro abstracts away a lot of the pain of interacting with pointers to structs, such as casting, pointer arithmetic, transmutation, etc.

A big inspiration for need for this crate was minimizing stack space for functions which use WinAPI structs, as they are often massive in size.


## Features

- Generate pointer structs with field offset access methods
- Support for arrays of pointers with indexing
- Pointer reinterpretation capabilities
- Safe and ergonomic field access

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
pstruct = "0.1.0"
```

## Example

```rust
use pstruct::p_struct;

// Define a byte array to simulate struct data
let byte_array: &[u8] = &[
    69,                     // `field_b` (offset 0)
    255, 255, 255, 255,     // `field_a` (offset 1)
    10, 20, 30, 40, 50, 60, 70, 80, 90, 100, // `field_d` (offset 5)
    40,                     // `field_c` (offset 0xF)
];

// Define a pointer struct using the macro
p_struct! {
    pub struct Example {
        #[offset(0x1)]
        field_a: u32,
        #[offset(0x0)]
        field_b: u8,
        #[offset(0xF, reinterpret)]
        field_c: *const u8,
        #[offset(0x5, array(10))]
        field_d: *const u8,
    }
}

let example_ptr = PExample::from(byte_array);

// Access fields
unsafe {
    assert_eq!(example_ptr.field_b(), 69);
    assert_eq!(example_ptr.field_a(), u32::MAX);
    
    // Array access
    let field_d_1 = example_ptr.get_field_d(0).unwrap();
    assert_eq!(*field_d_1, 10u8);
    
    // Reconstruct array
    let array = core::slice::from_raw_parts(example_ptr.field_d(), 10);
    assert_eq!(array, [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]);
    
    // Reinterpreted pointer
    assert_eq!(*(example_ptr.field_c().as_ref().unwrap()), 40u8);
}
```

## Attributes

### `offset`

The main attribute for specifying field offsets and behavior:

- Basic usage: `#[offset(0x1)]` - Specifies the offset from base address
- Reinterpret: `#[offset(0x1, reinterpret)]` - Reinterprets the pointer at the given offset as another pointer type.
- Array: `#[offset(0x1, array(size))]` - Defines an array of pointers and generates a getter method for indexing into the array.

## Safety

This crate involves heavy unsafe operations when accessing fields. Consumers of this crate are responsible for:
- Ensuring correct memory layout, alignment, bounds, etc.
- Ensuring offsets are valid for the data structure and all memory is readable.
- Proper lifetime management when constructing a PStruct from a raw pointer. *Lifetimes are enforced for you when creating a PStruct from a &[T]*.
- Bounds checking when accessing array fields.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## Notes
<sup>1</sup> You could always use references to prevent the copy, but this still doesn't solve the problem of manually defining structs with padding bytes which gets very tedious and is very common for those working with WinAPI (PEB, TEB, etc).