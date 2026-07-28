//! `#[derive(Component)]` for `renzora_plugin`.
//!
//! Generates everything the engine needs to store and edit a component it has no
//! Rust type for: the type path that serves as its identity, the field schema
//! the inspector renders, and a constructor for a default-valued instance.
//!
//! Never depend on this crate directly — `renzora_plugin` re-exports the derive.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type, parse_macro_input};

/// Map a Rust field type to the closed set of kinds the editor can draw.
///
/// Returning `None` skips the field rather than failing the build: a component
/// may legitimately hold data the inspector cannot edit, and refusing to compile
/// would force authors to split their types for the UI's benefit. Skipped fields
/// still exist and still round-trip — they are simply not editable.
fn field_kind(ty: &Type) -> Option<proc_macro2::TokenStream> {
    let Type::Path(p) = ty else { return None };
    let ident = p.path.segments.last()?.ident.to_string();
    Some(match ident.as_str() {
        "f32" => quote!(FieldKind::F32),
        "i32" | "u32" | "usize" | "i64" => quote!(FieldKind::I32),
        "bool" => quote!(FieldKind::Bool),
        "Vec3" => quote!(FieldKind::Vec3),
        "Quat" => quote!(FieldKind::Quat),
        _ => return None,
    })
}

#[proc_macro_derive(Component, attributes(component))]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let display = name.to_string();

    let Data::Struct(data) = &input.data else {
        return syn::Error::new_spanned(
            &input.ident,
            "#[derive(Component)] only supports structs — an enum has no stable \
             field layout for the inspector to address by offset",
        )
        .to_compile_error()
        .into();
    };

    let mut entries = Vec::new();
    if let Fields::Named(named) = &data.fields {
        for f in &named.named {
            let Some(kind) = field_kind(&f.ty) else { continue };
            let ident = f.ident.as_ref().unwrap();
            let fname = ident.to_string();
            entries.push(quote! {
                FieldDesc {
                    name: StrRef::new(#fname),
                    kind: #kind,
                    offset: ::core::mem::offset_of!(#name, #ident),
                }
            });
        }
    }

    let expanded = quote! {
        const _: () = {
            use ::renzora_plugin::sys::{ComponentDesc, FieldDesc, FieldKind, StrRef};

            impl ::renzora_plugin::ecs::Component for #name {
                // `module_path!()` makes the identity crate-qualified, so two
                // plugins can each define a `Spinner` without colliding. This
                // string is what scenes serialize — renaming the type or moving
                // it between modules breaks saved scenes, exactly as renaming a
                // Rust type would.
                const TYPE_PATH: &'static str =
                    ::core::concat!(::core::module_path!(), "::", ::core::stringify!(#name));

                fn display_name() -> &'static str { #display }

                fn fields() -> &'static [FieldDesc] {
                    static FIELDS: &[FieldDesc] = &[#(#entries),*];
                    FIELDS
                }

                fn descriptor() -> Option<ComponentDesc> {
                    // Writes a default-valued instance into host-provided
                    // storage. This is why the derive requires `Default`: the
                    // editor has to put *something* on the entity when you add
                    // the component, and zeroed memory is a bad answer — a
                    // `speed: 0.0` component is present, correct, and doing
                    // nothing, which reads as a broken plugin.
                    unsafe extern "C" fn init(out: *mut u8) {
                        out.cast::<#name>().write(<#name as ::core::default::Default>::default());
                    }
                    Some(ComponentDesc {
                        name: StrRef::new(
                            <#name as ::renzora_plugin::ecs::Component>::TYPE_PATH,
                        ),
                        size: ::core::mem::size_of::<#name>(),
                        align: ::core::mem::align_of::<#name>(),
                        drop: None,
                        display_name: StrRef::new(#display),
                        fields: <#name as ::renzora_plugin::ecs::Component>::fields().as_ptr(),
                        field_count:
                            <#name as ::renzora_plugin::ecs::Component>::fields().len(),
                        default_init: Some(init),
                    })
                }
            }
        };
    };
    expanded.into()
}
