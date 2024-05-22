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
                    pub extern "C" fn #snprintf_fn_name(&self, buffer: *mut u8, buflen: usize) -> i32 {
                        // If an encoding error occurs, a negative number is returned.
                        let s = match std::ffi::CString::new(format!("{:#?}", self)) {
                            Ok(s) => s,
                            Err(_) => return -1,
                        };

                        let bytes = s.as_bytes();
                        let len = bytes.len();

                        let written = len.min(buflen);
                        let slice = unsafe { std::slice::from_raw_parts_mut(buffer, written) };

                        // if there's space, the null will be copied from the CString directly
                        slice.copy_from_slice(&bytes[..written]);

                        // snprintf always null-terminates the buffer
                        if len > buflen {
                            slice[len] = 0;
                        }

                        // The number of characters that would have been written
                        // if n had been sufficiently large, not counting the
                        // terminating null character.
                        //
                        // When this returned value is non-negative and less
                        // than n, the string has been completely written.
                        (len - 1).try_into().unwrap()
                    }
                }
            }
        }
        _ => panic!("Snprintf can only be derived for structs and enums"),
    };

    gen.into()
}
