use darling::FromDeriveInput;
use darling::FromVariant;
use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, ToTokens};
use syn::{parse_macro_input, Data, DeriveInput, Field, Fields, Ident, Variant};

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

#[derive(Debug, FromDeriveInput, FromVariant)]
// #[derive(Debug, FromDeriveInput)]
#[darling(attributes(c_api))]
struct WrapperOpts {
    ident: Ident,
    attrs: Vec<syn::Attribute>,
    prefix: Option<String>,
    repr_c: Option<bool>,
    variant_prefix: Option<String>,
}

#[proc_macro_derive(CApiWrapper, attributes(c_api))]
pub fn c_api_wrapper_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let opts = match WrapperOpts::from_derive_input(&input) {
        Ok(v) => v,
        Err(e) => return TokenStream::from(e.write_errors()),
    };

    let name = &opts.ident;

    let prefix = opts
        .prefix
        .expect("Must have a prefix, Cbindgen does not support namespaces");
    let new_name = Ident::new(&format!("{}{}", prefix, name), name.span());

    let repr_c_token = if opts.repr_c.unwrap_or(false) {
        quote!(#[repr(C)])
    } else {
        quote!()
    };
    let expanded = match &input.data {
        Data::Struct(data) => {
            let field_names: Vec<_> = data
                .fields
                .iter()
                .map(|Field { ident, .. }| ident.clone())
                .collect();
            let fields: Vec<_> = data.fields.iter().map(|f| quote! { #f }).collect();

            quote! {
                #[cfg(feature = "c_api")]
                #repr_c_token
                #[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
                pub(crate) struct #new_name {
                    #(#fields),*
                }

                #[cfg(feature = "c_api")]
                impl From<#name> for #new_name {
                    fn from(item: #name) -> Self {
                        #new_name {
                            #(#field_names: item.#field_names),*
                        }
                    }
                }

                #[cfg(feature = "c_api")]
                impl From<#new_name> for #name {
                    fn from(item: #new_name) -> Self {
                        #name {
                            #(#field_names: item.#field_names),*
                        }
                    }
                }
            }
        }
        Data::Enum(data) => {
            let variants: Vec<TokenStream2> = data
                .variants
                .iter()
                .map(|variant| {
                    let opts = match WrapperOpts::from_variant(variant) {
                        Ok(v) => v,
                        Err(e) => return e.write_errors(),
                    };

                    let variant_prefix = opts.variant_prefix.unwrap_or_else(|| prefix.clone());
                    let ident = variant.ident.clone();
                    // // panic!("{:?}", ident);
                    // let new_ty_ident = format_ident!("{}{}", variant_prefix.trim(), ident);
                    // let new_ty = quote! { #new_ty_ident };

                    match &variant.fields {
                        Fields::Unit => quote! { #ident },
                        Fields::Unnamed(fields) => {
                            let fields: Vec<_> = fields
                                .unnamed
                                .iter()
                                .map(|field| {
                                    let inner_data_type = &field.ty;
                                    let inner_data_type_string =
                                        quote! { #inner_data_type }.to_string();
                                    let new_ty_ident = format_ident!(
                                        "{}{}",
                                        variant_prefix.trim(),
                                        inner_data_type_string,
                                    );
                                    let new_ty = quote! { #new_ty_ident };
                                    new_ty
                                })
                                .collect();
                            quote! { #ident(#(#fields),*) }
                        }
                        Fields::Named(fields) => {
                            let fields: Vec<_> = fields
                                .named
                                .iter()
                                .map(|Field { ident, ty, .. }| {
                                    let ident = match ident {
                                        Some(ident) => ident,
                                        None => return quote! { #ty },
                                    };
                                    let new_ty_ident =
                                        format_ident!("{}{}", variant_prefix.trim(), ident);
                                    let new_ty = quote! { #new_ty_ident };
                                    quote! { #ident: #new_ty }
                                })
                                .collect();
                            quote! { #ident { #(#fields),* } }
                        }
                    }
                })
                .collect();

            let from_old_match_arms: Vec<_> = data
                .variants
                .iter()
                .map(|Variant { ident, fields, .. }| match fields {
                    Fields::Unit => quote! { #name::#ident => #new_name::#ident },
                    _ => quote! { #name::#ident(v) => #new_name::#ident(v.into()) },
                })
                .collect();

            let from_new_match_arms: Vec<_> = data
                .variants
                .iter()
                .map(|Variant { ident, fields, .. }| match fields {
                    Fields::Unit => quote! { #new_name::#ident => #name::#ident },
                    _ => quote! { #new_name::#ident(v) => #name::#ident(v.into()) },
                })
                .collect();

            quote! {
                    #[cfg(feature = "c_api")]
                    #repr_c_token
                    #[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
                    pub(crate) enum #new_name {
                        #(#variants),*
                    }

                    #[cfg(feature = "c_api")]
                    impl From<#name> for #new_name {
                        fn from(item: #name) -> Self {
                            match item {
                                #(#from_old_match_arms),*
                            }
                        }
                    }

                    #[cfg(feature = "c_api")]
                    impl From<#new_name> for #name {
                        fn from(item: #new_name) -> Self {
                            match item {
                                #(#from_new_match_arms),*
                            }
                        }
                    }
            }
        }

        _ => panic!("Only works on structs and enums"),
    };

    // panic!("{}", TokenStream::from(expanded).to_string());
    TokenStream::from(expanded)
}
