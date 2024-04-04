use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput};

#[proc_macro_derive(Printf)]
pub fn derive_printf(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;
    let printf_fn_name = format_ident!("{}_printf", name);

    let gen = match &ast.data {
        Data::Struct(_) | Data::Enum(_) => {
            quote! {
                impl #name {
                    #[no_mangle]
                    pub extern "C" fn #printf_fn_name(&self) {
                        println!("{:#?}", self);
                    }
                }
            }
        }
        _ => panic!("Printf can only be derived for structs and enums"),
    };

    gen.into()
}

#[proc_macro_derive(Snprintf)]
pub fn derive_snprintf(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;
    let snprintf_fn_name = format_ident!("{}_snprintf", name);

    let gen = match &ast.data {
        Data::Struct(_) | Data::Enum(_) => {
            quote! {
                impl #name {
                    #[no_mangle]
                    pub extern "C" fn #snprintf_fn_name(&self, buffer: *mut u8, buflen: usize) -> usize {
                        let s = format!("{:#?}", self);
                        let bytes = s.as_bytes();
                        let len = bytes.len().min(buflen);
                        let slice = unsafe { std::slice::from_raw_parts_mut(buffer, len) };
                        slice.copy_from_slice(&bytes[..len]);
                        len
                    }
                }
            }
        }
        _ => panic!("Snprintf can only be derived for structs and enums"),
    };

    gen.into()
}
