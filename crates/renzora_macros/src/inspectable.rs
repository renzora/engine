use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::field_parse::{infer_field_type, title_case, FieldAttrs};

pub fn derive_inspectable(input: DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = &input.ident;

    // Parse #[inspectable(...)] attributes
    let mut display_name = title_case(&struct_name.to_string());
    let mut icon = "CUBE".to_string();
    let mut category = "component".to_string();
    let mut type_id: Option<String> = None;

    for attr in &input.attrs {
        if !attr.path().is_ident("inspectable") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value = meta.value()?;
                let lit: syn::Lit = value.parse()?;
                if let syn::Lit::Str(s) = lit {
                    display_name = s.value();
                }
            } else if meta.path.is_ident("icon") {
                let value = meta.value()?;
                let lit: syn::Lit = value.parse()?;
                if let syn::Lit::Str(s) = lit {
                    icon = s.value();
                }
            } else if meta.path.is_ident("category") {
                let value = meta.value()?;
                let lit: syn::Lit = value.parse()?;
                if let syn::Lit::Str(s) = lit {
                    category = s.value();
                }
            } else if meta.path.is_ident("type_id") {
                let value = meta.value()?;
                let lit: syn::Lit = value.parse()?;
                if let syn::Lit::Str(s) = lit {
                    type_id = Some(s.value());
                }
            }
            Ok(())
        })?;
    }

    let type_id = type_id.unwrap_or_else(|| {
        struct_name.to_string().chars()
            .enumerate()
            .fold(String::new(), |mut acc, (i, c)| {
                if c.is_uppercase() && i > 0 {
                    acc.push('_');
                }
                acc.push(c.to_ascii_lowercase());
                acc
            })
    });

    let fields = match &input.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(f) => &f.named,
            _ => return Err(syn::Error::new_spanned(&input.ident, "Inspectable only supports named fields")),
        },
        _ => return Err(syn::Error::new_spanned(&input.ident, "Inspectable only supports structs")),
    };

    // Generate field definitions
    let mut field_defs = Vec::new();
    for field in fields {
        let field_ident = field.ident.as_ref().unwrap();
        let field_attrs = FieldAttrs::from_field(field)?;

        if field_attrs.skip {
            continue;
        }

        // Skip padding and enabled fields (for post-process structs)
        let field_name_str = field_ident.to_string();
        if field_name_str.starts_with("_p") || field_name_str == "enabled" {
            continue;
        }

        let display = field_attrs.name.unwrap_or_else(|| title_case(&field_name_str));

        if field_attrs.readonly {
            field_defs.push(quote! {
                renzora_editor::FieldDef {
                    name: #display,
                    field_type: renzora_editor::FieldType::ReadOnly,
                    get_fn: |world, entity| {
                        world.get::<#struct_name>(entity)
                            .map(|s| renzora_editor::FieldValue::ReadOnly(format!("{:?}", s.#field_ident)))
                    },
                    set_fn: |_world, _entity, _val| {},
                }
            });
            continue;
        }

        let ft = infer_field_type(&field.ty);
        let field_type_expr = match ft {
            "Float" => {
                let speed = field_attrs.speed.unwrap_or(0.01);
                let min = field_attrs.min.unwrap_or(f32::MIN);
                let max = field_attrs.max.unwrap_or(f32::MAX);
                quote! { renzora_editor::FieldType::Float { speed: #speed, min: #min, max: #max } }
            }
            "Bool" => quote! { renzora_editor::FieldType::Bool },
            "Vec3" => {
                let speed = field_attrs.speed.unwrap_or(0.1);
                quote! { renzora_editor::FieldType::Vec3 { speed: #speed } }
            }
            "String" => quote! { renzora_editor::FieldType::String },
            "Color" => quote! { renzora_editor::FieldType::Color },
            _ => quote! { renzora_editor::FieldType::ReadOnly },
        };

        let get_fn = match ft {
            "Float" => quote! {
                |world, entity| world.get::<#struct_name>(entity).map(|s| renzora_editor::FieldValue::Float(s.#field_ident))
            },
            "Bool" => quote! {
                |world, entity| world.get::<#struct_name>(entity).map(|s| renzora_editor::FieldValue::Bool(s.#field_ident))
            },
            "Vec3" => quote! {
                |world, entity| world.get::<#struct_name>(entity).map(|s| renzora_editor::FieldValue::Vec3(s.#field_ident.into()))
            },
            "String" => quote! {
                |world, entity| world.get::<#struct_name>(entity).map(|s| renzora_editor::FieldValue::String(s.#field_ident.clone()))
            },
            "Color" => quote! {
                |world, entity| world.get::<#struct_name>(entity).map(|s| renzora_editor::FieldValue::Color(s.#field_ident.into()))
            },
            _ => quote! {
                |world, entity| world.get::<#struct_name>(entity).map(|s| renzora_editor::FieldValue::ReadOnly(format!("{:?}", s.#field_ident)))
            },
        };

        let set_fn = match ft {
            "Float" => quote! {
                |world, entity, val| {
                    if let renzora_editor::FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<#struct_name>(entity) { s.#field_ident = v; }
                    }
                }
            },
            "Bool" => quote! {
                |world, entity, val| {
                    if let renzora_editor::FieldValue::Bool(v) = val {
                        if let Some(mut s) = world.get_mut::<#struct_name>(entity) { s.#field_ident = v; }
                    }
                }
            },
            "Vec3" => quote! {
                |world, entity, val| {
                    if let renzora_editor::FieldValue::Vec3(v) = val {
                        if let Some(mut s) = world.get_mut::<#struct_name>(entity) { s.#field_ident = v.into(); }
                    }
                }
            },
            "String" => quote! {
                |world, entity, val| {
                    if let renzora_editor::FieldValue::String(v) = val {
                        if let Some(mut s) = world.get_mut::<#struct_name>(entity) { s.#field_ident = v; }
                    }
                }
            },
            "Color" => quote! {
                |world, entity, val| {
                    if let renzora_editor::FieldValue::Color(v) = val {
                        if let Some(mut s) = world.get_mut::<#struct_name>(entity) { s.#field_ident = v.into(); }
                    }
                }
            },
            _ => quote! { |_world, _entity, _val| {} },
        };

        field_defs.push(quote! {
            renzora_editor::FieldDef {
                name: #display,
                field_type: #field_type_expr,
                get_fn: #get_fn,
                set_fn: #set_fn,
            }
        });
    }

    // Use the icon as a constant path (e.g., regular::HEART)
    let icon_ident = syn::Ident::new(&icon, proc_macro2::Span::call_site());

    Ok(quote! {
        impl renzora_editor::InspectableComponent for #struct_name {
            fn inspector_entry() -> renzora_editor::InspectorEntry {
                renzora_editor::InspectorEntry {
                    type_id: #type_id,
                    display_name: #display_name,
                    icon: egui_phosphor::regular::#icon_ident,
                    category: #category,
                    has_fn: |world, entity| world.get::<#struct_name>(entity).is_some(),
                    add_fn: Some(|world, entity| { world.entity_mut(entity).insert(#struct_name::default()); }),
                    remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<#struct_name>(); }),
                    is_enabled_fn: None,
                    set_enabled_fn: None,
                    fields: vec![#(#field_defs),*],
                    custom_ui_fn: None,
                }
            }
        }
    })
}
