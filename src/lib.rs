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
