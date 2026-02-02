extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(JwcSerializable)]
pub fn jwc_serializable_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let serialization_logic = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => {
                let field_conversions = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    let name_str = name.as_ref().unwrap().to_string();
                    quote! {
                         if let Some(last) = members.last_mut() {
                             last.value.comma = true;
                         }
                         members.push(jwc::ObjectEntry::new(
                             #name_str.to_string(),
                             jwc::Node::new(self.#name.to_jwc())
                         ));
                    }
                });

                quote! {
                    let mut members: Vec<jwc::ObjectEntry> = Vec::new();
                    #(#field_conversions)*
                    jwc::Value::Object(members)
                }
            }
            _ => panic!("JwcSerializable only supports named fields structs"),
        },
        _ => panic!("JwcSerializable only supports structs"),
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

#[proc_macro_derive(JwcDeserializable)]
pub fn jwc_deserializable_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let deserialization_logic = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => {
                let field_parsing = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    let name_str = name.as_ref().unwrap().to_string();
                    let ty = &f.ty;
                    quote! {
                        #name: {
                            if let Some(entry) = map.remove(#name_str) {
                                <#ty as jwc::JwcDeserializable>::from_jwc(entry.value.value)?
                            } else {
                                return Err(format!("Missing field: {}", #name_str));
                            }
                        },
                    }
                });

                quote! {
                     if let jwc::Value::Object(members) = value {
                        let mut map: std::collections::HashMap<String, jwc::ObjectEntry> = std::collections::HashMap::new();
                        for entry in members {
                            map.insert(entry.key.clone(), entry);
                        }

                        Ok(#name {
                            #(#field_parsing)*
                        })
                    } else {
                        Err("Expected Object for struct".to_string())
                    }
                }
            }
            _ => panic!("JwcDeserializable only supports named fields structs"),
        },
        _ => panic!("JwcDeserializable only supports structs"),
    };

    let expanded = quote! {
        impl jwc::JwcDeserializable for #name {
            fn from_jwc(value: jwc::Value) -> Result<Self, String> {
                #deserialization_logic
            }
        }
    };

    TokenStream::from(expanded)
}
