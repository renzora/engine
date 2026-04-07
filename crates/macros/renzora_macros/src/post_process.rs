use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse::ParseStream, Fields, ItemStruct, Token, Lit};

use crate::field_parse::{infer_field_type, title_case, FieldAttrs};

/// Parsed attributes from `#[post_process(shader = "...", name = "...", icon = "...")]`.
struct PostProcessAttrs {
    shader: String,
    name: Option<String>,
    icon: String,
    category: String,
    type_id: Option<String>,
}

impl PostProcessAttrs {
    fn parse(attr: &proc_macro2::TokenStream) -> syn::Result<Self> {
        let mut shader = String::new();
        let mut name = None;
        let mut icon = "SPARKLE".to_string();
        let mut category = "post_process".to_string();
        let mut type_id = None;

        // Parse as attribute args
        syn::parse::Parser::parse2(
            |input: ParseStream| {
                while !input.is_empty() {
                    let ident: syn::Ident = input.parse()?;
                    input.parse::<Token![=]>()?;
                    let lit: Lit = input.parse()?;
                    if let Lit::Str(s) = &lit {
                        match ident.to_string().as_str() {
                            "shader" => shader = s.value(),
                            "name" => name = Some(s.value()),
                            "icon" => icon = s.value(),
                            "category" => category = s.value(),
                            "type_id" => type_id = Some(s.value()),
                            _ => {}
                        }
                    }
                    let _ = input.parse::<Token![,]>();
                }
                Ok(())
            },
            attr.clone(),
        )?;

        if shader.is_empty() {
            return Err(syn::Error::new_spanned(attr, "post_process requires `shader = \"...\"`"));
        }

        Ok(Self { shader, name, icon, category, type_id })
    }
}

pub fn post_process_attr(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let pp_attrs = PostProcessAttrs::parse(&attr)?;
    let input: ItemStruct = syn::parse2(item)?;
    let struct_name = &input.ident;
    let vis = &input.vis;

    let display_name = pp_attrs.name.unwrap_or_else(|| title_case(&struct_name.to_string()));
    let type_id = pp_attrs.type_id.unwrap_or_else(|| {
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
    // Strip "Settings" suffix from type_id if present
    let type_id = type_id.strip_suffix("_settings").unwrap_or(&type_id).to_string();
    let shader_path = &pp_attrs.shader;
    let icon = &pp_attrs.icon;
    let category = &pp_attrs.category;

    // Collect user fields
    let user_fields = match &input.fields {
        Fields::Named(f) => f.named.iter().collect::<Vec<_>>(),
        _ => return Err(syn::Error::new_spanned(&input.ident, "post_process only supports named fields")),
    };

    // Count user fields to determine padding needed.
    // GPU uniform buffers must be 16-byte aligned (4 f32s). We pad to a minimum
    // of 8 f32s (2 vec4s) to match the existing convention across all effects.
    let user_f32_count = user_fields.len(); // each user field = 1 f32
    let total_with_enabled = user_f32_count + 1; // +1 for enabled
    let min_total = 8usize; // minimum 2 vec4s
    let target = if total_with_enabled <= min_total {
        min_total
    } else {
        // Round up to next multiple of 4
        ((total_with_enabled + 3) / 4) * 4
    };
    let padding_count = target - total_with_enabled;

    // Generate user field declarations (with original attributes stripped of #[field(...)])
    let user_field_decls: Vec<_> = user_fields.iter().map(|f| {
        let ident = &f.ident;
        let ty = &f.ty;
        let vis = &f.vis;
        // Keep non-field attributes
        let attrs: Vec<_> = f.attrs.iter().filter(|a| !a.path().is_ident("field")).collect();
        quote! { #(#attrs)* #vis #ident: #ty }
    }).collect();

    // Generate padding fields (serde skip so scene files aren't affected by layout changes)
    let padding_fields: Vec<_> = (0..padding_count).map(|i| {
        let name = syn::Ident::new(&format!("_padding{}", i + 1), proc_macro2::Span::call_site());
        quote! {
            #[serde(skip, default)]
            pub #name: f32
        }
    }).collect();

    // Generate Default impl (respects #[field(default = ...)])
    let user_defaults: Vec<_> = user_fields.iter().map(|f| {
        let ident = &f.ident;
        let field_attrs = FieldAttrs::from_field(f).unwrap_or_default();
        if let Some(val) = field_attrs.default {
            let val = val as f32;
            quote! { #ident: #val }
        } else {
            quote! { #ident: Default::default() }
        }
    }).collect();
    let padding_defaults: Vec<_> = (0..padding_count).map(|i| {
        let name = syn::Ident::new(&format!("_padding{}", i + 1), proc_macro2::Span::call_site());
        quote! { #name: 0.0 }
    }).collect();

    // Build the embedded shader path
    let crate_name = std::env::var("CARGO_PKG_NAME").unwrap_or_default();
    let shader_embed_path = format!("embedded://{}/{}", crate_name, shader_path);

    // Generate inspector field defs for user fields
    let mut inspector_field_defs = Vec::new();
    for field in &user_fields {
        let field_ident = field.ident.as_ref().unwrap();
        let field_attrs = FieldAttrs::from_field(field)?;
        if field_attrs.skip {
            continue;
        }

        let field_name_str = field_ident.to_string();
        let display = field_attrs.name.unwrap_or_else(|| title_case(&field_name_str));

        if field_attrs.readonly {
            inspector_field_defs.push(quote! {
                renzora::editor::FieldDef {
                    name: #display,
                    field_type: renzora::editor::FieldType::ReadOnly,
                    get_fn: |world, entity| {
                        world.get::<#struct_name>(entity)
                            .map(|s| renzora::editor::FieldValue::ReadOnly(format!("{:?}", s.#field_ident)))
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
                quote! { renzora::editor::FieldType::Float { speed: #speed, min: #min, max: #max } }
            }
            "Bool" => quote! { renzora::editor::FieldType::Bool },
            "Vec3" => {
                let speed = field_attrs.speed.unwrap_or(0.1);
                quote! { renzora::editor::FieldType::Vec3 { speed: #speed } }
            }
            "String" => quote! { renzora::editor::FieldType::String },
            "Color" => quote! { renzora::editor::FieldType::Color },
            _ => quote! { renzora::editor::FieldType::ReadOnly },
        };

        let get_fn = match ft {
            "Float" => quote! {
                |world, entity| world.get::<#struct_name>(entity).map(|s| renzora::editor::FieldValue::Float(s.#field_ident))
            },
            "Bool" => quote! {
                |world, entity| world.get::<#struct_name>(entity).map(|s| renzora::editor::FieldValue::Bool(s.#field_ident))
            },
            _ => quote! {
                |world, entity| world.get::<#struct_name>(entity).map(|s| renzora::editor::FieldValue::ReadOnly(format!("{:?}", s.#field_ident)))
            },
        };

        let set_fn = match ft {
            "Float" => quote! {
                |world, entity, val| {
                    if let renzora::editor::FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<#struct_name>(entity) { s.#field_ident = v; }
                    }
                }
            },
            "Bool" => quote! {
                |world, entity, val| {
                    if let renzora::editor::FieldValue::Bool(v) = val {
                        if let Some(mut s) = world.get_mut::<#struct_name>(entity) { s.#field_ident = v; }
                    }
                }
            },
            _ => quote! { |_world, _entity, _val| {} },
        };

        inspector_field_defs.push(quote! {
            renzora::editor::FieldDef {
                name: #display,
                field_type: #field_type_expr,
                get_fn: #get_fn,
                set_fn: #set_fn,
            }
        });
    }

    let icon_ident = syn::Ident::new(icon, proc_macro2::Span::call_site());

    // Keep any non-post_process attributes from the original struct
    let kept_attrs: Vec<_> = input.attrs.iter().filter(|a| {
        !a.path().is_ident("post_process")
    }).collect();

    Ok(quote! {
        #(#kept_attrs)*
        #[derive(Component, Clone, Copy, Reflect, serde::Serialize, serde::Deserialize,
                 bevy::render::render_resource::ShaderType, bevy::render::extract_component::ExtractComponent)]
        #[reflect(Component, Serialize, Deserialize)]
        #[extract_component_filter(With<Camera3d>)]
        #vis struct #struct_name {
            #(#user_field_decls,)*
            #(#padding_fields,)*
            pub enabled: f32,
        }

        impl Default for #struct_name {
            fn default() -> Self {
                Self {
                    #(#user_defaults,)*
                    #(#padding_defaults,)*
                    enabled: 1.0,
                }
            }
        }

        impl renzora_postprocess::PostProcessEffect for #struct_name {
            fn fragment_shader() -> bevy::shader::ShaderRef {
                #shader_embed_path.into()
            }
        }

        #[cfg(feature = "editor")]
        impl renzora::editor::InspectableComponent for #struct_name {
            fn inspector_entry() -> renzora::editor::InspectorEntry {
                renzora::editor::InspectorEntry {
                    type_id: #type_id,
                    display_name: #display_name,
                    icon: renzora::egui_phosphor::regular::#icon_ident,
                    category: #category,
                    has_fn: |world, entity| world.get::<#struct_name>(entity).is_some(),
                    add_fn: Some(|world, entity| { world.entity_mut(entity).insert(#struct_name::default()); }),
                    remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<#struct_name>(); }),
                    is_enabled_fn: Some(|world, entity| {
                        world.get::<#struct_name>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)
                    }),
                    set_enabled_fn: Some(|world, entity, val| {
                        if let Some(mut s) = world.get_mut::<#struct_name>(entity) {
                            s.enabled = if val { 1.0 } else { 0.0 };
                        }
                    }),
                    fields: vec![#(#inspector_field_defs),*],
                    custom_ui_fn: None,
                }
            }
        }
    })
}
