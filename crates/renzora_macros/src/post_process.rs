use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse::ParseStream, Fields, ItemStruct, Lit, Token};

use crate::field_parse::{infer_field_type, title_case, FieldAttrs};

/// Parsed attributes from `#[post_process(shader = "...", name = "...", icon = "...")]`.
struct PostProcessAttrs {
    shader: String,
    name: Option<String>,
    icon: String,
    category: String,
    type_id: Option<String>,
    /// Sort key within `LdrPost`. `None` leaves the trait's `0.0` default.
    order: Option<f32>,
    /// `snapshot = true` asks the pipeline for the two extra binding slots
    /// (`@binding(3)` texture + `@binding(4)` sampler) holding an auto-captured
    /// copy of the previous fully-composited frame. A shader that names those
    /// bindings without this crashes wgpu at pipeline creation: "Shader global
    /// ResourceBinding { group: 0, binding: 3 } is not available in the pipeline
    /// layout".
    snapshot: bool,
    /// The expression `freeze_snapshot` returns, written over `self` — e.g.
    /// `"self.progress < 1.0"`. While it is true the snapshot stops refreshing,
    /// so the shader can blend the frozen outgoing frame against the live one.
    frozen_when: Option<String>,
}

impl PostProcessAttrs {
    fn parse(attr: &proc_macro2::TokenStream) -> syn::Result<Self> {
        let mut shader = String::new();
        let mut name = None;
        let mut icon = "SPARKLE".to_string();
        let mut category = "post_process".to_string();
        let mut type_id = None;
        let mut order = None;
        let mut snapshot = false;
        let mut frozen_when = None;

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
                            "frozen_when" => frozen_when = Some(s.value()),
                            _ => {}
                        }
                    } else if let Lit::Bool(b) = &lit {
                        if ident == "snapshot" {
                            snapshot = b.value();
                        }
                    } else if ident == "order" {
                        // A number, not a string, so it is matched here rather
                        // than in the `Lit::Str` arm above. Both literal forms
                        // are accepted: `order = 1` is the obvious way to write
                        // it and would otherwise be silently ignored.
                        order = match &lit {
                            Lit::Float(f) => Some(f.base10_parse::<f32>()?),
                            Lit::Int(i) => Some(i.base10_parse::<i32>()? as f32),
                            _ => None,
                        };
                    }
                    let _ = input.parse::<Token![,]>();
                }
                Ok(())
            },
            attr.clone(),
        )?;

        if shader.is_empty() {
            return Err(syn::Error::new_spanned(
                attr,
                "post_process requires `shader = \"...\"`",
            ));
        }

        Ok(Self {
            shader,
            name,
            icon,
            category,
            type_id,
            order,
            snapshot,
            frozen_when,
        })
    }
}

pub fn post_process_attr(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let pp_attrs = PostProcessAttrs::parse(&attr)?;
    let input: ItemStruct = syn::parse2(item)?;
    let struct_name = &input.ident;
    let vis = &input.vis;

    let display_name = pp_attrs
        .name
        .unwrap_or_else(|| title_case(&struct_name.to_string()));
    let type_id = pp_attrs.type_id.unwrap_or_else(|| {
        struct_name
            .to_string()
            .chars()
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
    let type_id = type_id
        .strip_suffix("_settings")
        .unwrap_or(&type_id)
        .to_string();
    let shader_path = &pp_attrs.shader;
    let icon = &pp_attrs.icon;
    let category = &pp_attrs.category;

    // Collect user fields
    let user_fields = match &input.fields {
        Fields::Named(f) => f.named.iter().collect::<Vec<_>>(),
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "post_process only supports named fields",
            ))
        }
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
        total_with_enabled.div_ceil(4) * 4
    };
    let padding_count = target - total_with_enabled;

    // Generate user field declarations (with original attributes stripped of #[field(...)])
    let user_field_decls: Vec<_> = user_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let ty = &f.ty;
            let vis = &f.vis;
            // Keep non-field attributes
            let attrs: Vec<_> = f
                .attrs
                .iter()
                .filter(|a| !a.path().is_ident("field"))
                .collect();
            quote! { #(#attrs)* #vis #ident: #ty }
        })
        .collect();

    // Generate padding fields (serde skip so scene files aren't affected by layout changes)
    let padding_fields: Vec<_> = (0..padding_count)
        .map(|i| {
            let name = syn::Ident::new(
                &format!("_padding{}", i + 1),
                proc_macro2::Span::call_site(),
            );
            quote! {
                #[serde(skip, default)]
                pub #name: f32
            }
        })
        .collect();

    // Generate Default impl (respects #[field(default = ...)])
    let user_defaults: Vec<_> = user_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let ty = &f.ty;
            let field_attrs = FieldAttrs::from_field(f).unwrap_or_default();
            if let Some(val) = field_attrs.default {
                // `default = 9` on a `u32` field has to land as a `u32`. Emitting
                // it as an `f32` the way every other field takes it is an
                // `expected u32, found f32` pointing at the attribute rather than
                // at the field, which is a long way from the line to change.
                if crate::field_parse::infer_field_type(ty) == "Int" {
                    let val = val as i64;
                    quote! { #ident: #val as #ty }
                } else {
                    let val = val as f32;
                    quote! { #ident: #val }
                }
            } else {
                quote! { #ident: Default::default() }
            }
        })
        .collect();
    let padding_defaults: Vec<_> = (0..padding_count)
        .map(|i| {
            let name = syn::Ident::new(
                &format!("_padding{}", i + 1),
                proc_macro2::Span::call_site(),
            );
            quote! { #name: 0.0 }
        })
        .collect();

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
        let display = field_attrs
            .name
            .unwrap_or_else(|| title_case(&field_name_str));

        if field_attrs.readonly {
            inspector_field_defs.push(quote! {
                renzora::FieldDef {
                    name: #display,
                    field_type: renzora::FieldType::ReadOnly,
                    get_fn: |world, entity| {
                        world.get::<#struct_name>(entity)
                            .map(|s| renzora::FieldValue::ReadOnly(format!("{:?}", s.#field_ident)))
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
                quote! { renzora::FieldType::Float { speed: #speed, min: #min, max: #max } }
            }
            // `Int` carries no speed: the widget snaps its model to whole
            // numbers, and a fractional drag model fighting a rounded re-read
            // makes the value stutter backwards mid-drag.
            "Int" => {
                let min = field_attrs.min.unwrap_or(0.0);
                let max = field_attrs.max.unwrap_or(f32::MAX);
                quote! { renzora::FieldType::Int { min: #min, max: #max } }
            }
            "Bool" => quote! { renzora::FieldType::Bool },
            "Vec3" => {
                let speed = field_attrs.speed.unwrap_or(0.1);
                quote! { renzora::FieldType::Vec3 { speed: #speed } }
            }
            "String" => quote! { renzora::FieldType::String },
            "Color" => quote! { renzora::FieldType::Color },
            _ => quote! { renzora::FieldType::ReadOnly },
        };

        let get_fn = match ft {
            "Float" => quote! {
                |world, entity| world.get::<#struct_name>(entity).map(|s| renzora::FieldValue::Float(s.#field_ident))
            },
            "Bool" => quote! {
                |world, entity| world.get::<#struct_name>(entity).map(|s| renzora::FieldValue::Bool(s.#field_ident))
            },
            // The registry has ONE numeric wire type, so an integer travels as a
            // `Float` and is rounded back on the way in.
            "Int" => quote! {
                |world, entity| world.get::<#struct_name>(entity).map(|s| renzora::FieldValue::Float(s.#field_ident as f32))
            },
            _ => quote! {
                |world, entity| world.get::<#struct_name>(entity).map(|s| renzora::FieldValue::ReadOnly(format!("{:?}", s.#field_ident)))
            },
        };

        let set_fn = match ft {
            "Float" => quote! {
                |world, entity, val| {
                    if let renzora::FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<#struct_name>(entity) { s.#field_ident = v; }
                    }
                }
            },
            "Bool" => quote! {
                |world, entity, val| {
                    if let renzora::FieldValue::Bool(v) = val {
                        if let Some(mut s) = world.get_mut::<#struct_name>(entity) { s.#field_ident = v; }
                    }
                }
            },
            "Int" => {
                let field_ty = &field.ty;
                quote! {
                    |world, entity, val| {
                        if let renzora::FieldValue::Float(v) = val {
                            if let Some(mut s) = world.get_mut::<#struct_name>(entity) {
                                s.#field_ident = v.round() as #field_ty;
                            }
                        }
                    }
                }
            }
            _ => quote! { |_world, _entity, _val| {} },
        };

        inspector_field_defs.push(quote! {
            renzora::FieldDef {
                name: #display,
                field_type: #field_type_expr,
                get_fn: #get_fn,
                set_fn: #set_fn,
            }
        });
    }

    // Emit the icon as the kebab-case name string the native (bevy_ui) inspector
    // resolves to a Phosphor glyph (e.g. "sparkle").
    let icon_name = crate::field_parse::icon_kebab(icon);

    // Emitted only when the attribute asked for one, so an effect that says
    // nothing inherits the trait's `0.0` rather than having it restated here.
    let order_fn = match pp_attrs.order {
        Some(o) => quote! { fn order() -> f32 { #o } },
        None => quote! {},
    };

    let snapshot_fns = if pp_attrs.snapshot {
        // Parsed rather than pasted, so a malformed predicate is a syntax error
        // pointing at the attribute instead of at the generated impl.
        let frozen = match &pp_attrs.frozen_when {
            Some(src) => syn::parse_str::<syn::Expr>(src).map_err(|e| {
                syn::Error::new_spanned(
                    &input.ident,
                    format!("post_process `frozen_when` is not an expression: {e}"),
                )
            })?,
            // No predicate: the snapshot tracks the live frame forever, which is
            // the same picture as not having one. Almost certainly a mistake, but
            // it renders rather than crashing.
            None => syn::parse_quote!(false),
        };
        quote! {
            fn has_extra_texture() -> bool { true }
            fn extra_texture_is_snapshot() -> bool { true }
            fn freeze_snapshot(&self) -> bool { #frozen }
        }
    } else {
        quote! {}
    };

    // Keep any non-post_process attributes from the original struct
    let kept_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("post_process"))
        .collect();

    Ok(quote! {
        #(#kept_attrs)*
        // `renzora::serde`, never a bare `serde`. A native plugin is compiled
        // against the SDK with no manifest of its own to resolve `serde` from,
        // and a plugin that did add its own would get a SECOND copy: `Vec3`
        // implements the engine's `Serialize`, so deriving against a private
        // copy does not compile. The contract crate re-exports the engine's for
        // exactly this reason.
        #[derive(Component, Clone, Copy, Reflect,
                 renzora::serde::Serialize, renzora::serde::Deserialize,
                 bevy::render::render_resource::ShaderType, bevy::render::extract_component::ExtractComponent)]
        #[serde(crate = "renzora::serde")]
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

        // `renzora::postprocess`, not the `renzora_postprocess` shim. A plugin
        // depends on the contract crate alone, and the shim is an engine crate
        // it has no way to reach. Both paths name the same trait.
        impl renzora::postprocess::PostProcessEffect for #struct_name {
            fn fragment_shader() -> bevy::shader::ShaderRef {
                #shader_embed_path.into()
            }
            #order_fn
            #snapshot_fns
        }

        // Deliberately NOT `#[cfg(feature = "editor")]`. A native plugin has no
        // cargo features at all — rustc compiles it directly against the SDK with
        // none set — so the gate would be false and the effect's whole inspector
        // section would silently vanish. Registering unconditionally is also just
        // correct: the entry goes into a registry resource a shipped game never
        // reads, so it costs one `Vec` push at startup.
        impl renzora::InspectableComponent for #struct_name {
            // Field min/max bounds are user-supplied float literals that may
            // approximate math constants (e.g. TAU); they're emitted verbatim.
            #[allow(clippy::approx_constant)]
            fn inspector_entry() -> renzora::InspectorEntry {
                renzora::InspectorEntry {
                    type_id: #type_id,
                    display_name: #display_name,
                    icon: #icon_name,
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
                }
            }
        }
    })
}
