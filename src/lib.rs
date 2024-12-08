//! # pstruct
//!
//! A Rust procedural macro for generating pointer struct implementations with field offset access. The purpose of this crate is to minimize stack space, preventing struct copies<sup>1</sup>, while still allowing ergonomic field access. This macro abstracts away a lot of the pain of interacting with pointers to structs, such as casting, pointer arithmetic, transmutation, etc.
//!
//! A big inspiration for need for this crate was minimizing stack space for functions which use WinAPI structs, as they are often massive in size.
//!
//!
//! ## Features
//!
//! - Generate pointer structs with field offset access methods
//! - Support for arrays of pointers with indexing
//! - Pointer reinterpretation capabilities
//! - Safe and ergonomic field access
//!
//! ## Usage
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! pstruct = "0.1.0"
//! ```
//!
//! ## Example
//!
//! ```rust
//! use pstruct::p_struct;
//!
//! // Define a byte array to simulate struct data
//! let byte_array: &[u8] = &[
//!     69,                     // `field_b` (offset 0)
//!     255, 255, 255, 255,     // `field_a` (offset 1)
//!     10, 20, 30, 40, 50, 60, 70, 80, 90, 100, // `field_d` (offset 5)
//!     40,                     // `field_c` (offset 0xF)
//! ];
//!
//! // Define a pointer struct using the macro
//! p_struct! {
//!     pub struct Example {
//!         #[offset(0x1)]
//!         field_a: u32,
//!         #[offset(0x0)]
//!         field_b: u8,
//!         #[offset(0xF, reinterpret)]
//!         field_c: *const u8,
//!         #[offset(0x5, array(10))]
//!         field_d: *const u8,
//!     }
//! }
//!
//! let example_ptr = PExample::from(byte_array);
//!
//! // Access fields
//! unsafe {
//!     assert_eq!(example_ptr.field_b(), 69);
//!     assert_eq!(example_ptr.field_a(), u32::MAX);
//!     
//!     // Array access
//!     let field_d_1 = example_ptr.get_field_d(0).unwrap();
//!     assert_eq!(*field_d_1, 10u8);
//!     
//!     // Reconstruct array
//!     let array = core::slice::from_raw_parts(example_ptr.field_d(), 10);
//!     assert_eq!(array, [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]);
//!     
//!     // Reinterpreted pointer
//!     assert_eq!(*(example_ptr.field_c().as_ref().unwrap()), 40u8);
//! }
//! ```
//!
//! ## Attributes
//!
//! ### `offset`
//!
//! The main attribute for specifying field offsets and behavior:
//!
//! - Basic usage: `#[offset(0x1)]` - Specifies the offset from base address
//! - Reinterpret: `#[offset(0x1, reinterpret)]` - Reinterprets the pointer at the given offset as another pointer type.
//! - Array: `#[offset(0x1, array(size))]` - Defines an array of pointers and generates a getter method for indexing into the array.
//!
//! ## Safety
//!
//! This crate involves heavy unsafe operations when accessing fields. Consumers of this crate are responsible for:
//! - Ensuring correct memory layout, alignment, bounds, etc.
//! - Ensuring offsets are valid for the data structure and all memory is readable.
//! - Proper lifetime management when constructing a PStruct from a raw pointer. *Lifetimes are enforced for you when creating a PStruct from a &[T]*.
//! - Bounds checking when accessing array fields.
//!
//! ## License
//!
//! Licensed under either of
//!
//!  * Apache License, Version 2.0
//!    ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
//!  * MIT license
//!    ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
//!
//! at your option.
//!
//! ## Contribution
//!
//! Unless you explicitly state otherwise, any contribution intentionally submitted
//! for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
//! dual licensed as above, without any additional terms or conditions.
//!
//! ## Notes
//! <sup>1</sup> You could always use references to prevent the copy, but this still doesn't solve the problem of manually defining structs with padding bytes which gets very tedious and is very common for those working with WinAPI (PEB, TEB, etc).
extern crate proc_macro;

mod types;
use types::*;

use attribute_derive::FromAttr;
use proc_macro::TokenStream;

macro_rules! dbg_print {
    ($($arg:tt)*) => {
        #[cfg(feature = "debug")]
        eprintln!("[offset]: {}", format_args!($($arg)*));
    };
}

#[proc_macro]
/// Generates a pointer struct from a struct definition.
///
/// The original struct definition is discarded, while a new struct is generated with a `P` prefix.
/// This new struct contains methods for reading the fields of the pointer struct.
/// Clone and Copy derives are added if they don't exist, and the generated type is `#[repr(transparent)]` to allow for transmutation from pointers.
///
/// ## Offset Attribute
///
/// The `offset` attribute is used to specify the offset of the field from the base address.
/// This offset is added to the base address to return the field.
///
/// **Default behavior for generated offset getters is to use [read_unaligned][core::ptr::read_unaligned] to read the field, copying it's value and returning it from the function.**
///
/// ### Sub attributes:
/// - `reinterpret`: Overrides default behavior, reinterprets the returned pointer from base address + offset as a different pointer type.
/// - `array`: Allows you to define the field as an array of pointers. This generates a method which allows for indexing into the array.
///
///
/// # Example
///
/// ```rust
/// use pstruct::p_struct;
///
///   // Define a byte array to simulate struct data.
///   let byte_array: &[u8] = &[
///     69, // `field_b` (offset 0)
///     255, 255, 255, 255, // `field_a` (offset 1)
///     10, 20, 30, 40, 50, 60, 70, 80, 90, 100, // `field_d` (offset 5)
///     40,  // `field_c` (offset 0xF)
/// ];
///
/// // Define a pointer struct using the macro.
/// p_struct! {
///     pub struct Example {
///         #[offset(0x1)]
///         field_a: u32,
///         #[offset(0x0)]
///         field_b: u8,
///         #[offset(0xF, reinterpret)]
///         // Reinterprets the pointer from base + 0xF (via transmute) as a *const u8
///         // Use this feature to return pointers to a value rather than the value itself.
///         field_c: *const u8,
///         #[offset(0x5, array(10))]
///         // Defines the field as an array of pointers, allowing for indexing into the array.
///         // This is useful for returning arrays of pointers.
///         field_d: *const u8,
///     }
/// }
///
/// let example_ptr = PExample::from(byte_array);
/// assert_eq!(unsafe { example_ptr.field_b() }, 69);
/// assert_eq!(unsafe { example_ptr.field_a() }, u32::MAX);
///
/// let field_d_1 = unsafe { example_ptr.get_field_d(0).unwrap() };
/// assert_eq!(unsafe { *field_d_1 }, 10u8);
/// let reconstruct_array = unsafe { core::slice::from_raw_parts(example_ptr.field_d(), 10) };
/// assert_eq!(reconstruct_array, [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]);
/// assert_eq!(unsafe { *(example_ptr.field_c().as_ref().unwrap()) }, 40u8);
///
///
/// ```
pub fn p_struct(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match p_struct_impl(input) {
        Ok(ts) => ts,
        Err(e) => e.into_compile_error().into(),
    }
}

fn p_struct_impl(input: TokenStream) -> syn::Result<TokenStream> {
    let mut named_struct = syn::parse::<NamedStruct>(input)?;
    // Fix the attributes to ensure Clone and Copy derives are added if they don't exist
    named_struct.fix_attrs()?;

    let mut methods: Vec<proc_macro2::TokenStream> =
        Vec::with_capacity(named_struct.fields.named.len());

    for named_field in &named_struct.fields.named {
        // Always safe to unwrap, because we know the struct has named fields
        let field_name = unsafe { named_field.ident.as_ref().unwrap_unchecked().clone() };
        let field_type = named_field.ty.clone();
        dbg_print!(
            "Field Name: {} | Field Type: {}",
            field_name,
            field_type.to_token_stream().to_string()
        );

        let offset_attr = named_field
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("offset"));

        if let Some(offset) = offset_attr {
            let offset_attr = OffsetAttr::from_attribute(offset)?;
            offset_attr.is_valid(named_field)?;
            let ts = offset_attr.to_token_stream(&field_name, &field_type);
            methods.push(ts.into());
        } else {
            return Err(syn::Error::new_spanned(
                named_field,
                "Unknown attribute found. Offset is the only valid attribute for this macro.",
            ));
        }
    }
    Ok(named_struct.into_token_stream(methods))
}
