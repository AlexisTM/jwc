//! Derive macros for `jwc::JwcSerializable` and `jwc::JwcDeserializable`.
//!
//! Supported field attributes (all under `#[jwc(...)]`):
//!
//! - `rename = "x"` — use `"x"` as the JSON key instead of the field name.
//! - `default` — on deserialize, if the field is missing, use `Default::default()`.
//! - `skip` — skip the field on both serialize and deserialize (requires `Default` for deserialize).
//! - `skip_serializing` — omit from serialized output only.
//! - `skip_deserializing` — ignore in input; requires a default.

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, LitStr, parse_macro_input};

#[derive(Default)]
struct FieldAttrs {
    rename: Option<String>,
    default: bool,
    skip_ser: bool,
    skip_de: bool,
}

fn parse_field_attrs(attrs: &[syn::Attribute]) -> FieldAttrs {
    let mut out = FieldAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("jwc") {
            continue;
        }
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                let s: LitStr = meta.value()?.parse()?;
                out.rename = Some(s.value());
            } else if meta.path.is_ident("default") {
                out.default = true;
            } else if meta.path.is_ident("skip") {
                out.skip_ser = true;
                out.skip_de = true;
            } else if meta.path.is_ident("skip_serializing") {
                out.skip_ser = true;
            } else if meta.path.is_ident("skip_deserializing") {
                out.skip_de = true;
            }
            Ok(())
        });
    }
    out
}

#[proc_macro_derive(JwcSerializable, attributes(jwc))]
pub fn jwc_serializable_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let serialization_logic = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => {
                let field_conversions = fields.named.iter().filter_map(|f| {
                    let attrs = parse_field_attrs(&f.attrs);
                    if attrs.skip_ser {
                        return None;
                    }
                    let ident = &f.ident;
                    let key = attrs
                        .rename
                        .unwrap_or_else(|| ident.as_ref().unwrap().to_string());
                    Some(quote! {
                        members.push(jwc::ObjectEntry::new(
                            #key.to_string(),
                            jwc::Node::new(self.#ident.to_jwc()),
                        ));
                    })
                });

                quote! {
                    let mut members: ::std::vec::Vec<jwc::ObjectEntry> = ::std::vec::Vec::new();
                    #(#field_conversions)*
                    jwc::Value::Object(members)
                }
            }
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "JwcSerializable only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "JwcSerializable only supports structs")
                .to_compile_error()
                .into();
        }
    };

    let expanded = quote! {
        impl jwc::JwcSerializable for #name {
            fn to_jwc(&self) -> jwc::Value {
                #serialization_logic
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(JwcDeserializable, attributes(jwc))]
pub fn jwc_deserializable_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let struct_name_str = name.to_string();

    let deserialization_logic = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => {
                let field_parsing = fields.named.iter().map(|f| {
                    let attrs = parse_field_attrs(&f.attrs);
                    let ident = &f.ident;
                    let ident_str = ident.as_ref().unwrap().to_string();
                    let key = attrs.rename.clone().unwrap_or_else(|| ident_str.clone());
                    let ty = &f.ty;
                    if attrs.skip_de {
                        // Skipped fields require a Default.
                        quote! {
                            #ident: ::core::default::Default::default(),
                        }
                    } else if attrs.default {
                        quote! {
                            #ident: match map.remove(#key) {
                                Some(entry) => <#ty as jwc::JwcDeserializable>::from_jwc(entry.value.value)?,
                                None => ::core::default::Default::default(),
                            },
                        }
                    } else {
                        quote! {
                            #ident: match map.remove(#key) {
                                Some(entry) => <#ty as jwc::JwcDeserializable>::from_jwc(entry.value.value)?,
                                None => return ::core::result::Result::Err(
                                    jwc::Error::missing_field(#key),
                                ),
                            },
                        }
                    }
                });

                quote! {
                    match value {
                        jwc::Value::Object(members) => {
                            let mut map: ::std::collections::HashMap<::std::string::String, jwc::ObjectEntry>
                                = ::std::collections::HashMap::with_capacity(members.len());
                            for entry in members {
                                map.insert(entry.key.clone(), entry);
                            }
                            ::core::result::Result::Ok(#name { #(#field_parsing)* })
                        }
                        other => ::core::result::Result::Err(jwc::Error::ty_at(
                            "object",
                            jwc::_value_kind(&other),
                            #struct_name_str,
                        )),
                    }
                }
            }
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "JwcDeserializable only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "JwcDeserializable only supports structs")
                .to_compile_error()
                .into();
        }
    };

    let expanded = quote! {
        impl jwc::JwcDeserializable for #name {
            fn from_jwc(value: jwc::Value) -> jwc::Result<Self> {
                #deserialization_logic
            }
        }
    };

    TokenStream::from(expanded)
}
