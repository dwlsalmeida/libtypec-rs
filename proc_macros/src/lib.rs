use darling::FromDeriveInput;
use darling::FromField;
use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::format_ident;
use quote::quote;
use syn::parse_macro_input;
use syn::Data;
use syn::DeriveInput;
use syn::Field;
use syn::Fields;
use syn::Ident;
use syn::Variant;

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(c_api))]
struct WrapperOpts {
    ident: Ident,
    prefix: String,
    repr_c: bool,
    #[darling(default)]
    manual_from_impl: bool,
}

#[derive(Debug, FromField)]
#[darling(attributes(c_api))]
struct FieldOpts {
    #[darling(default)]
    no_prefix: bool,
    rename_type: Option<String>,
    /// Convert the type into ManuallyDrop<Box<T>>. These types are totally
    /// opaque to C and must provide a way for C to free them.
    #[darling(default)]
    opaque: bool,
}

/// Derive a C API wrapper for a struct or enum.
///
/// Use `repr_c` to annotate the wrapper as repr(C). Only types that are
/// strictly composed of other repr(C) types should have `repr_c=true`. Types
/// that are not marked repr(C) will be forward declared by cbindgen.
///
/// A prefix must be used to uniquely identify the new type, as cbindgen is not
/// aware of Rust namespaces.
///
/// The wrappers can be converted from/to its native Rust types with from() and
/// into().
#[proc_macro_derive(CApiWrapper, attributes(c_api))]
pub fn c_api_wrapper_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let opts = match WrapperOpts::from_derive_input(&input) {
        Ok(v) => v,
        Err(e) => return TokenStream::from(e.write_errors()),
    };

    let name = &opts.ident;
    let new_name = Ident::new(&format!("{}{}", &opts.prefix, name), name.span());

    let repr_c_token = if opts.repr_c {
        quote!(#[repr(C)])
    } else {
        quote!()
    };

    let derives = quote!(#[derive(Debug, Clone, PartialEq)]);

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
                    .map(|f| prefix_struct_field_types(&opts, f))
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
                        #derives
                        pub(crate) struct #new_name {
                            #(#fields),*
                        }

                        #from_impl
                    }
                } else {
                    quote! {
                        #[cfg(feature = "c_api")]
                        #repr_c_token
                        #derives
                        pub(crate) struct #new_name {
                            #(#fields),*
                        }
                    }
                }
            }
            Fields::Unnamed(fields) => {
                let field_types: Vec<_> = fields
                    .unnamed
                    .iter()
                    .map(|f| prefix_struct_field_types(&opts, f))
                    .collect();
                let field_indices: Vec<_> =
                    (0..fields.unnamed.len()).map(syn::Index::from).collect();

                quote! {
                    #[cfg(feature = "c_api")]
                    #repr_c_token
                    #derives
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
            _ => panic!("Unit structs not supported"),
        },
        Data::Enum(data) => {
            let variants: Vec<TokenStream2> = data
                .variants
                .iter()
                .map(prefix_enum_variants)
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
                    #derives
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

// These types are not prefixed.
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

fn prefix_struct_field_types(opts: &WrapperOpts, f: &Field) -> TokenStream2 {
    let field_opt = FieldOpts::from_field(f).unwrap();

    let Field { ident, ty, .. } = f;
    let ty_string = quote! { #ty }.to_string();

    // Any non-primitive type is prefixed by default.
    if let Some(new_name) = field_opt.rename_type {
        let new_name = syn::Type::from_string(&new_name).unwrap();
        if field_opt.opaque {
            quote! {pub(crate) #ident: std::mem::ManuallyDrop<Box<#new_name>> }
        } else {
            quote! {pub(crate) #ident: #new_name }
        }
    } else if !is_whitelisted_type(&ty_string) && !field_opt.no_prefix {
        let new_ty_ident = format_ident!("{}{}", &opts.prefix, ty_string);
        let new_ty = quote! { #new_ty_ident };
        if field_opt.opaque {
            quote! { pub(crate) #ident: std::mem::ManuallyDrop<Box<#new_ty>> }
        } else {
            quote! { pub(crate) #ident: #new_ty }
        }
    } else if field_opt.opaque {
        quote! { pub(crate) #ident: std::mem::ManuallyDrop<Box<#ty>> }
    } else {
        quote! { pub(crate) #ident: #ty }
    }
}

// Enum variants must use a type alias to be named the same as the C type.
// A "c_api::" prefix is appended to disambiguate the wrapper from the alias.
//
// See libtypec_rs::Pdo for examples.
fn prefix_enum_variants(variant: &Variant) -> TokenStream2 {
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
}
