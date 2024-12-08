use attribute_derive::FromAttr;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_quote, Attribute, Fields, FieldsNamed, Ident, ItemStruct, Type, Visibility};

pub struct NamedStruct {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub fields: FieldsNamed,
}

impl syn::parse::Parse for NamedStruct {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let item_struct = input.call(ItemStruct::parse)?;
        let fields_named = match &item_struct.fields {
            Fields::Named(fields) => fields,
            _ => {
                return Err(syn::Error::new_spanned(
                    item_struct,
                    "Struct must have named fields",
                ))
            }
        };

        let has_generics = !item_struct.generics.params.is_empty();
        if has_generics {
            return Err(syn::Error::new_spanned(
                item_struct,
                "Structs with generics are not supported",
            ));
        }

        Ok(NamedStruct {
            attrs: item_struct.attrs,
            vis: item_struct.vis,
            name: item_struct.ident,
            fields: fields_named.clone(),
        })
    }
}

impl NamedStruct {
    pub fn fix_attrs(&mut self) -> syn::Result<()> {
        let mut has_derive = false;
        let mut has_copy = false;
        let mut has_clone = false;

        for attr in &self.attrs {
            if attr.path().is_ident("derive") {
                has_derive = true;
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("Clone") {
                        has_clone = true;
                    } else if meta.path.is_ident("Copy") {
                        has_copy = true;
                    }
                    Ok(())
                })?;
            }
        }

        // Add Clone and Copy derives if they don't exist
        if has_derive {
            let mut new_attrs = Vec::new();
            for attr in self.attrs.clone() {
                if attr.path().is_ident("derive") {
                    let mut derives = vec![];
                    if has_copy {
                        derives.push(quote!(Copy));
                    }
                    if has_clone {
                        derives.push(quote!(Clone));
                    }
                    if !has_copy {
                        derives.push(quote!(Copy));
                    }
                    if !has_clone {
                        derives.push(quote!(Clone));
                    }
                    new_attrs.push(parse_quote!(#[derive(#(#derives),*)]));
                } else {
                    new_attrs.push(attr);
                }
            }
            self.attrs = new_attrs;
        } else {
            // No derive attribute exists, add a new one
            self.attrs.push(parse_quote!(#[derive(Clone, Copy)]));
        }
        Ok(())
    }

    pub fn into_token_stream(self, methods: Vec<proc_macro2::TokenStream>) -> TokenStream {
        let methods = methods.into_iter();

        let name = format_ident!("P{}", self.name);
        let static_name = format_ident!("SP{}", self.name);
        let attrs = self.attrs;
        let vis = self.vis;

        let expanded = quote! {
            #vis type #static_name = #name<'static>;
            #(#attrs)*
            #vis struct #name<'ptr_lifetime>(usize, core::marker::PhantomData<&'ptr_lifetime ()>);

            impl<'ptr_lifetime> #name<'ptr_lifetime> {
                /// Creates a new pointer from a base address
                fn new<T>(base: *mut T) -> #name<'ptr_lifetime> {
                    Self(base.addr(), core::marker::PhantomData)
                }
                /// Determines if the pointer is null
                pub fn is_null(&self) -> bool {
                    self.0 == 0
                }

                /// Returns the address of the pointer
                pub fn addr(&self) -> usize {
                    self.0
                }

                #(#methods)*
            }

            /* Default from impls using 'static lifetimes for pointers which don't contain lifetime information */
            impl<T> From<*mut T> for #name<'static> {
                fn from(value: *mut T) -> #name<'static> {
                    #name::new(value)
                }
            }

            impl <T> From<*const T> for #name<'static> {
                fn from(value: *const T) -> #name<'static> {
                    #name::new(value as *mut T)
                }
            }

            /* From impls for slices which contain lifetime information */
            impl <'a, T> From<&'a [T]> for #name<'a> {
                fn from(value: &'a [T]) -> #name<'a> {
                    #name::new(value.as_ptr() as *mut T)
                }
            }
        };
        expanded.into()
    }
}
#[derive(Debug, FromAttr)]
#[attribute(ident = offset)]
#[attribute(error(
    missing_field = "Required field \"{field}\" not specified",
    conflict = "Cannot use both reinterpret and array attributes together"
))]
pub struct OffsetAttr {
    #[from_attr(positional)]
    pub offset: usize,
    #[from_attr(optional, conflicts = [reinterpret])]
    pub array: Option<usize>,
    #[from_attr(optional, conflicts = [array])]
    pub reinterpret: bool,
}

impl OffsetAttr {
    /// Returns true if the attribute is set to treat the field as an array
    pub fn is_array(&self) -> bool {
        self.array.is_some()
    }

    /// Returns true if the attribute is valid..
    ///
    /// Currently this just does a check to make sure the array size is not 0, as it's expected to be > 0.
    pub fn is_valid<T>(&self, span: T) -> syn::Result<()>
    where
        T: quote::ToTokens,
    {
        if self.array.is_some_and(|s| s == 0) {
            return Err(syn::Error::new_spanned(span, "Array size cannot be 0"));
        }
        Ok(())
    }

    /// Converts an OffsetAttr with a given field name/type into a TokenStream of methods
    pub fn to_token_stream(&self, field_name: &Ident, field_type: &Type) -> TokenStream {
        let offset = self.offset;
        let read_expr = if self.reinterpret || self.array.is_some_and(|s| s != 0) {
            quote! {
                // let ptr_with_addr: *mut u8 = core::ptr::without_provenance_mut(self.0 + #offset_lit as usize);
                // Base usize + Offset usize = pointer to bytes
                let ptr_with_addr: *mut u8 = (self.0 + #offset as usize) as *mut u8;
                core::mem::transmute(ptr_with_addr)
            }
        } else {
            quote! {
                //let ptr_with_addr: *mut u8 = core::ptr::without_provenance_mut(self.0 + #offset_lit as usize);
                let ptr_with_addr: *mut u8 = (self.0 + #offset as usize) as *mut u8;
                core::ptr::read_unaligned(ptr_with_addr as *const #field_type)
            }
        };

        // Generate the array getter method if the field is an array
        let array_method = if self.is_array() {
            // SAFETY: This is safe because we check if the array size is not 0 in the is_valid method
            let array_size = unsafe { self.array.unwrap_unchecked() };
            let getter_name = format_ident!("get_{}", field_name.to_string().to_lowercase());
            Some(quote! {
                /// Retrieves a pointer to an element in the array, given the index.
                ///
                /// Returns `None` if the index specified is out of bounds for the array.
                pub unsafe fn #getter_name(&self, index: usize) -> Option<#field_type> {
                    if index >= #array_size {
                        return None;
                    }
                    let base_array_ptr = self.#field_name().addr();
                    let final_addr = base_array_ptr + (index * #array_size as usize);
                    let final_ptr = final_addr as *mut u8;
                    Some(core::mem::transmute(final_ptr))
                }
            })
        } else {
            None
        };

        // If the field is not an array, make the getter public
        // Otherwise, make it private
        let visibility_modifier = if !self.is_array() {
            Some(quote! {pub})
        } else {
            None
        };
        quote! {
            #visibility_modifier unsafe fn #field_name(&self) -> #field_type {
                #read_expr
            }
            #array_method
        }
        .into()
    }
}
