use darling::FromDeriveInput;
use darling::FromField;
use darling::FromMeta;
use darling::FromVariant;
use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, ToTokens};
use syn::Type;
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
    #[darling(default)]
    manual_from_impl: bool,
}

#[derive(Debug, FromField)]
#[darling(attributes(c_api))]
struct FieldOpts {
    ident: Option<Ident>,
    attrs: Vec<syn::Attribute>,
    ty: Type,
    #[darling(default)]
    no_prefix: bool,
    rename_type: Option<String>,
}

/// Derive a C API wrapper for a struct or enum.
///
/// Use `repr_c=true` to annotate the wrapper as repr(C). Only types that are
/// strictly composed of other repr(C) types should have `repr_c=true`. Types
/// that are not marked repr(C) will be forward declared by cbindgen.
///
/// A prefix must be used to uniquely identify the new type, as cbindgen is not
/// aware of Rust namespaces.
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
        Data::Struct(data) => match &data.fields {
            Fields::Named(_) => {
                let field_names: Vec<_> = data
                    .fields
                    .iter()
                    .map(|Field { ident, .. }| ident.clone())
                    .collect();

                let fields: Vec<_> = data
                    .fields
                    .iter()
                    .map(|f| {
                        let field_opt = FieldOpts::from_field(f).unwrap();
                        let Field { ident, ty, .. } = f;
                        let ty_string = quote! { #ty }.to_string();
                        // Any non-primitive type is prefixed by default.
                        if let Some(new_name) = field_opt.rename_type {
                            let new_name = syn::Type::from_string(&new_name).unwrap();
                            quote! {#ident: #new_name }
                        } else if !is_whitelisted_type(&ty_string) && !field_opt.no_prefix {
                            let new_ty_ident = format_ident!("{}{}", prefix, ty_string);
                            let new_ty = quote! { #new_ty_ident };
                            quote! { #ident: #new_ty }
                        } else {
                            quote! { #ident: #ty }
                        }
                    })
                    .collect();

                let from_impl = quote! {
                    #[cfg(feature = "c_api")]
                    impl From<#name> for #new_name {
                        fn from(item: #name) -> Self {
                            #new_name {
                                #(#field_names: item.#field_names.into()),*
                            }
                        }
                    }

                    #[cfg(feature = "c_api")]
                    impl From<#new_name> for #name {
                        fn from(item: #new_name) -> Self {
                            #name {
                                #(#field_names: item.#field_names.into()),*
                            }
                        }
                    }
                };

                if !opts.manual_from_impl {
                    quote! {
                        #[cfg(feature = "c_api")]
                        #repr_c_token
                        #[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
                        pub(crate) struct #new_name {
                            #(#fields),*
                        }

                        #from_impl
                    }
                } else {
                    quote! {
                        #[cfg(feature = "c_api")]
                        #repr_c_token
                        #[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
                        pub(crate) struct #new_name {
                            #(#fields),*
                        }
                    }
                }
            }
            Fields::Unnamed(fields) => {
                let field_types: Vec<_> = fields.unnamed.iter().map(|f| &f.ty).collect();
                let field_indices: Vec<_> =
                    (0..fields.unnamed.len()).map(syn::Index::from).collect();

                quote! {
                    #[cfg(feature = "c_api")]
                    #repr_c_token
                    #[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
                    pub(crate) struct #new_name(#(#field_types),*)

                    #[cfg(feature = "c_api")]
                    impl From<#name> for #new_name {
                        fn from(item: #name) -> Self {
                            #new_name(#(item.#field_indices),*)
                        }
                    }

                    #[cfg(feature = "c_api")]
                    impl From<#new_name> for #name {
                        fn from(item: #new_name) -> Self {
                            #name(#(item.#field_indices),*)
                        }
                    }
                }
            }
            _ => panic!("Structs must have named fields or unnamed fields"),
        },
        Data::Enum(data) => {
            let variants: Vec<TokenStream2> = data
                .variants
                .iter()
                .map(|variant| {
                    let ident = variant.ident.clone();

                    match &variant.fields {
                        Fields::Unit => quote! { #ident },
                        Fields::Unnamed(fields) => {
                            let fields: Vec<_> = fields
                                .unnamed
                                .iter()
                                .map(|field| {
                                    let inner_data_type = &field.ty;
                                    quote! { c_api::#inner_data_type }
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
                                        None => return quote! { c_api::#ty },
                                    };
                                    quote! { #ident: #ty }
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

    TokenStream::from(expanded)
}

fn is_whitelisted_type(ty_string: &str) -> bool {
    let whitelisted_types = [
        "bool",
        "char",
        "i8",
        "i16",
        "i32",
        "i64",
        "i128",
        "isize",
        "u8",
        "u16",
        "u32",
        "u64",
        "u128",
        "usize",
        "f32",
        "f64",
        "str",
        "String",
        "BcdWrapper",
        "Milliohm",
        "Millivolt",
        "Milliamp",
        "Milliwatt",
    ];
    whitelisted_types.contains(&ty_string)
}
