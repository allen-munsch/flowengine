use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Type};

struct FieldConfig {
    ident: syn::Ident,
    ty: Type,
    default: Option<String>,
    rename: Option<String>,
    skip: bool,
}

fn parse_field_config(field: &syn::Field) -> FieldConfig {
    let ident = field.ident.as_ref().unwrap().clone();
    let ty = field.ty.clone();
    let mut default = None;
    let mut rename = None;
    let mut skip = false;

    for attr in &field.attrs {
        if !attr.path().is_ident("config") {
            continue;
        }
        let result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                skip = true;
            } else if meta.path.is_ident("default") {
                let value = meta.value()?;
                let s = value.to_string();
                default = Some(s.trim_matches('"').to_string());
            } else if meta.path.is_ident("rename") {
                let value = meta.value()?;
                rename = Some(value.to_string().trim_matches('"').to_string());
            }
            Ok(())
        });
        if let Err(_e) = result {
            return FieldConfig { ident, ty, default: None, rename: None, skip: true };
        }
    }

    FieldConfig { ident, ty, default, rename, skip }
}

fn is_simple_type(ty: &Type, name: &str) -> bool {
    matches!(ty, Type::Path(ref tp) if tp.path.is_ident(name))
}

fn is_option_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(ref tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if seg.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(ref args) = seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

#[proc_macro_derive(NodeConfig, attributes(config))]
pub fn derive_node_config(input: TokenStream) -> TokenStream {
    let input_ast = syn::parse_macro_input!(input as DeriveInput);
    let struct_name = &input_ast.ident;

    let fields = match &input_ast.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(&input_ast, "NodeConfig only works on structs with named fields")
                    .to_compile_error()
                    .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input_ast, "NodeConfig can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    let configs: Vec<_> = fields.iter().map(parse_field_config).collect();

    let mut field_extractions = Vec::new();
    let mut field_assignments = Vec::new();

    for fc in &configs {
        if fc.skip {
            // Generate a default for skipped fields
            let ident = &fc.ident;
            let ty = &fc.ty;
            field_extractions.push(quote! {
                let #ident: #ty = Default::default();
            });
            field_assignments.push(quote! { #ident });
            continue;
        }

        let field_name = fc.rename.as_ref().unwrap_or(&fc.ident.to_string()).clone();
        let ident = &fc.ident;

        if let Some(inner) = is_option_type(&fc.ty) {
            // Handle Option<T> fields
            let inner_ty_str = quote!(#inner).to_string();
            let extraction: proc_macro2::TokenStream;

            if inner_ty_str == "String" {
                extraction = quote! {
                    let #ident: Option<String> = config.get(#field_name)
                        .and_then(|v| v.as_str().map(|s| s.to_string()));
                };
            } else if inner_ty_str == "f64" {
                extraction = quote! {
                    let #ident: Option<f64> = config.get(#field_name)
                        .and_then(|v| v.as_f64());
                };
            } else if inner_ty_str == "bool" {
                extraction = quote! {
                    let #ident: Option<bool> = config.get(#field_name)
                        .and_then(|v| v.as_bool());
                };
            } else if inner_ty_str == "u64" {
                extraction = quote! {
                    let #ident: Option<u64> = config.get(#field_name)
                        .and_then(|v| v.as_f64().map(|f| f as u64));
                };
            } else {
                extraction = quote! {
                    let #ident: Option<#inner> = config.get(#field_name)
                        .and_then(|v| v.as_str().and_then(|s| s.parse().ok()));
                };
            }
            field_extractions.push(extraction);
        } else {
            let has_default = fc.default.is_some();
            let field_name_str = &field_name;
            let ident = &fc.ident;

            if has_default {
                // Field with default value
                let default_val = fc.default.as_ref().unwrap();
                let default_tokens: proc_macro2::TokenStream = match default_val.parse() {
                    Ok(ts) => ts,
                    Err(_) => quote! { compile_error!("Invalid default value") },
                };

                if is_simple_type(&fc.ty, "String") {
                    field_extractions.push(quote! {
                        let #ident: String = match config.get(#field_name_str) {
                            Some(v) => flowcore::Value::as_str(v)
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| #default_tokens),
                            None => #default_tokens,
                        };
                    });
                } else if is_simple_type(&fc.ty, "f64") {
                    field_extractions.push(quote! {
                        let #ident: f64 = match config.get(#field_name_str) {
                            Some(v) => flowcore::Value::as_f64(v).unwrap_or(#default_tokens),
                            None => #default_tokens,
                        };
                    });
                } else if is_simple_type(&fc.ty, "bool") {
                    field_extractions.push(quote! {
                        let #ident: bool = match config.get(#field_name_str) {
                            Some(v) => flowcore::Value::as_bool(v).unwrap_or(#default_tokens),
                            None => #default_tokens,
                        };
                    });
                } else if is_simple_type(&fc.ty, "u64") {
                    field_extractions.push(quote! {
                        let #ident: u64 = match config.get(#field_name_str) {
                            Some(v) => flowcore::Value::as_f64(v)
                                .map(|f| f as u64).unwrap_or(#default_tokens),
                            None => #default_tokens,
                        };
                    });
                } else {
                    let ty = &fc.ty;
                    field_extractions.push(quote! {
                        let #ident: #ty = match config.get(#field_name_str) {
                            Some(v) => flowcore::Value::as_str(v)
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(#default_tokens),
                            None => #default_tokens,
                        };
                    });
                }
            } else {
                // Required field without default
                let extraction_code;
                if is_simple_type(&fc.ty, "String") {
                    extraction_code = quote! {
                        let #ident: String = config.get(#field_name_str)
                            .and_then(|v| flowcore::Value::as_str(v))
                            .ok_or_else(|| format!("Missing required field '{}'", #field_name_str))
                            .map(|s| s.to_string())?;
                    };
                } else if is_simple_type(&fc.ty, "f64") {
                    extraction_code = quote! {
                        let #ident: f64 = config.get(#field_name_str)
                            .and_then(|v| flowcore::Value::as_f64(v))
                            .ok_or_else(|| format!("Missing required field '{}'", #field_name_str))?;
                    };
                } else if is_simple_type(&fc.ty, "bool") {
                    extraction_code = quote! {
                        let #ident: bool = config.get(#field_name_str)
                            .and_then(|v| flowcore::Value::as_bool(v))
                            .ok_or_else(|| format!("Missing required field '{}'", #field_name_str))?;
                    };
                } else if is_simple_type(&fc.ty, "usize") {
                    extraction_code = quote! {
                        let #ident: usize = config.get(#field_name_str)
                            .and_then(|v| flowcore::Value::as_f64(v))
                            .ok_or_else(|| format!("Missing required field '{}'", #field_name_str))
                            .map(|f| f as usize)?;
                    };
                } else if is_simple_type(&fc.ty, "u64") {
                    extraction_code = quote! {
                        let #ident: u64 = config.get(#field_name_str)
                            .and_then(|v| flowcore::Value::as_f64(v))
                            .ok_or_else(|| format!("Missing required field '{}'", #field_name_str))
                            .map(|f| f as u64)?;
                    };
                } else {
                    let ty = &fc.ty;
                    extraction_code = quote! {
                        let #ident: #ty = config.get(#field_name_str)
                            .and_then(|v| flowcore::Value::as_str(v))
                            .ok_or_else(|| format!("Missing required field '{}'", #field_name_str))
                            .and_then(|s| s.parse::<#ty>().map_err(|e| format!("Invalid value for '{}': {}", #field_name_str, e)))?;
                    };
                }
                field_extractions.push(extraction_code);
            }
        }

        field_assignments.push(quote! { #ident });
    }

    let expanded = quote! {
        impl TryFrom<&std::collections::HashMap<String, flowcore::Value>> for #struct_name {
            type Error = String;

            fn try_from(config: &std::collections::HashMap<String, flowcore::Value>) -> Result<Self, Self::Error> {
                #(#field_extractions)*
                Ok(Self {
                    #(#field_assignments,)*
                })
            }
        }
    };

    TokenStream::from(expanded)
}
