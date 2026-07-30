//! `#[derive(Component)]` for `renzora_plugin`.
//!
//! Generates everything the engine needs to store and edit a component it has no
//! Rust type for: the type path that serves as its identity, the field schema
//! the inspector renders, and a constructor for a default-valued instance.
//!
//! Never depend on this crate directly — `renzora_plugin` re-exports the derive.

use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Spacing, TokenStream as TokenStream2, TokenTree};
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type, parse_macro_input};

/// Write BSN source as Rust tokens rather than as a string literal.
///
/// ```ignore
/// commands.spawn_scene(bsn! {
///     #Panel
///     Node { flex_direction: Column, row_gap: Px(8.0) }
///     BackgroundColor(Srgba(0.1, 0.1, 0.12, 1.0))
///     Children [
///         Text("Grid"),
///         ( Button Node { padding: Axes(Px(8.0), Px(3.0)) }
///           Children [ Text("Spawn") ] ),
///     ]
/// });
/// ```
///
/// The syntax is `bevy_scene`'s: components are space-separated, `#Name` names
/// the entity, and children hang off the relationship that owns them —
/// `Children [ … ]` — rather than off bare brackets. Parentheses group a
/// multi-component entity inside a list, where they are what says which
/// components belong to which entity.
///
/// The expansion is a `&'static str` — the same text you would have written by
/// hand, without the `r#"…"#`. What that buys is real tokens: an editor
/// highlights and indents it, unbalanced brackets are a compile error rather
/// than a runtime parse failure, and there is no escaping to get wrong.
///
/// Component *names* are still resolved at run time, by the host, against both
/// the type registry and the plugin schema registry. That is deliberate and is
/// what lets one syntax construct `Node` and your own components alike — but it
/// does mean a misspelled component name is a logged warning, not a compile
/// error. Bevy's own `bsn!` can check them because it expands to code that
/// constructs the types; this one cannot, because the types are on the other
/// side of an ABI boundary.
#[proc_macro]
pub fn bsn(input: TokenStream) -> TokenStream {
    let text = render(TokenStream2::from(input));
    // A `Scene`, not a bare `&str`, so the call site reads
    // `commands.spawn_scene(bsn! { … })` exactly as it does in Bevy.
    quote!(::renzora_plugin::ecs::Scene(#text)).into()
}

/// [`bsn!`] for a comma-separated list of entities.
#[proc_macro]
pub fn bsn_list(input: TokenStream) -> TokenStream {
    let text = render(TokenStream2::from(input));
    quote!(::renzora_plugin::ecs::Scene(#text)).into()
}

/// Reconstruct source text from tokens.
///
/// `TokenStream::to_string` would be shorter, but it separates every token with
/// a space — `-2.5` comes back as `- 2.5`, which RON then refuses to read as a
/// negative number. So spacing is decided here: joint punctuation stays joined,
/// and a space is emitted only where two tokens would otherwise merge into one.
fn render(stream: TokenStream2) -> String {
    let mut out = String::new();
    render_into(stream, &mut out);
    out
}

fn render_into(stream: TokenStream2, out: &mut String) {
    let mut prev_word = false;
    for tree in stream {
        match tree {
            TokenTree::Group(g) => {
                let (open, close) = match g.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Bracket => ("[", "]"),
                    // `None`-delimited groups come from macro expansion and have
                    // no source spelling; splice their contents.
                    Delimiter::None => ("", ""),
                };
                out.push_str(open);
                render_into(g.stream(), out);
                out.push_str(close);
                // A component's body is followed by the next component, and
                // `Transform{..}PointLight{..}` would read as one name.
                prev_word = true;
            }
            TokenTree::Ident(i) => {
                // Two idents in a row are separate components — `Button Node` —
                // and running them together would make one name.
                if prev_word {
                    out.push(' ');
                }
                out.push_str(&i.to_string());
                prev_word = true;
            }
            TokenTree::Literal(l) => {
                if prev_word {
                    out.push(' ');
                }
                out.push_str(&l.to_string());
                prev_word = true;
            }
            TokenTree::Punct(p) => {
                out.push(p.as_char());
                // `Joint` means the next token was adjacent in the source, which
                // is how `::` and `->` survive as one operator.
                prev_word = p.spacing() == Spacing::Alone && p.as_char() == ',';
                if prev_word {
                    out.push(' ');
                    prev_word = false;
                }
            }
        }
    }
}

/// Map a Rust field type to the closed set of kinds the editor can draw.
///
/// Returning `None` skips the field rather than failing the build: a component
/// may legitimately hold data the inspector cannot edit, and refusing to compile
/// would force authors to split their types for the UI's benefit. Skipped fields
/// still exist and still round-trip — they are simply not editable.
/// Parse `#[field(min = 0.0, max = 2.0, speed = 0.01)]` into a `sys::FieldRange`.
///
/// Mirrors the attribute the older `#[post_process]` macro already understood, so
/// converting an effect to the C ABI keeps its tuned inspector ranges instead of
/// silently demoting every slider to an unbounded drag.
///
/// `speed` is optional and defaults to 0, which the host reads as "pick one from
/// the range". `min`/`max` are both required if either appears — a half-specified
/// range has no sensible completion, and guessing one end is how a slider ends up
/// tuned to something nobody chose.
/// The `name = ".."` from `#[component(..)]` / `#[resource(..)]`, if given.
fn component_display_name(input: &DeriveInput) -> syn::Result<Option<String>> {
    let Some(attr) = input
        .attrs
        .iter()
        .find(|a| a.path().is_ident("component") || a.path().is_ident("resource"))
    else {
        return Ok(None);
    };
    let mut display = None;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("name") {
            let lit: syn::LitStr = meta.value()?.parse()?;
            display = Some(lit.value());
        }
        Ok(())
    })?;
    Ok(display)
}

fn field_attr(f: &syn::Field) -> syn::Result<(bool, Option<proc_macro2::TokenStream>)> {
    let Some(attr) = f.attrs.iter().find(|a| a.path().is_ident("field")) else {
        return Ok((false, None));
    };
    let (mut min, mut max, mut speed, mut skip) = (None, None, None, false);
    attr.parse_nested_meta(|meta| {
        // `skip` is a bare word, not `name = value`, so it has to be handled
        // before anything tries to parse a value.
        if meta.path.is_ident("skip") {
            skip = true;
            return Ok(());
        }
        let value: syn::LitFloat = meta.value()?.parse()?;
        let v: f32 = value.base10_parse()?;
        if meta.path.is_ident("min") {
            min = Some(v);
        } else if meta.path.is_ident("max") {
            max = Some(v);
        } else if meta.path.is_ident("speed") {
            speed = Some(v);
        } else {
            // `default` is accepted and ignored: the old macro used it, and the
            // C ABI gets defaults from `Default::default` via `default_init`
            // instead, so rejecting it would break a copy-paste conversion for no
            // benefit.
            let _ = meta.path.get_ident();
        }
        Ok(())
    })?;

    // A skipped field keeps its place in the struct — the layout has to match the
    // shader's uniform — it just gets no inspector row and no range.
    if skip {
        return Ok((true, None));
    }
    match (min, max) {
        (None, None) => Ok((false, None)),
        (Some(min), Some(max)) => {
            let speed = speed.unwrap_or(0.0);
            Ok((
                false,
                Some(quote! {
                    ::renzora_plugin::sys::FieldRange { min: #min, max: #max, speed: #speed }
                }),
            ))
        }
        _ => Err(syn::Error::new_spanned(
            attr,
            "#[field(..)] needs both `min` and `max`, or neither",
        )),
    }
}

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

/// A resource is registered from the same descriptor a component is — in Bevy a
/// resource is a component on a hidden entity — so the two derives differ only
/// in which trait they land on and whether the type is spawnable.
#[proc_macro_derive(Resource, attributes(resource, field))]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput), true)
}

#[proc_macro_derive(Component, attributes(component, field))]
pub fn derive_component(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput), false)
}

fn expand(input: DeriveInput, is_resource: bool) -> TokenStream {
    let name = &input.ident;
    // `#[component(name = "CRT")]` overrides the inspector label. The ident is a
    // poor default for an acronym — `Crt`, `Ascii`, `Oit` — and these labels are
    // user-facing, so a conversion that silently retitled forty effects would be a
    // visible regression.
    let display = match component_display_name(&input) {
        Ok(n) => n.unwrap_or_else(|| name.to_string()),
        Err(e) => return e.to_compile_error().into(),
    };

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
    let mut ranges = Vec::new();
    if let Fields::Named(named) = &data.fields {
        for f in &named.named {
            let Some(kind) = field_kind(&f.ty) else { continue };
            let ident = f.ident.as_ref().unwrap();
            let fname = ident.to_string();
            // `_`-prefixed fields are padding or private state, not something to
            // put a slider on. A uniform's `_pad: f32` showing up as an editable
            // row is noise at best and a way to corrupt alignment at worst.
            if fname.starts_with('_') {
                continue;
            }
            let (skip, range) = match field_attr(f) {
                Ok(v) => v,
                Err(e) => return e.to_compile_error().into(),
            };
            // Skipped fields still occupy their place in the struct — the layout
            // has to match the shader's uniform — they simply get no row.
            if skip {
                continue;
            }
            // The index into `fields()`, which is what `set_field_range` addresses.
            let index = entries.len();
            entries.push(quote! {
                FieldDesc {
                    name: StrRef::new(#fname),
                    kind: #kind,
                    offset: ::core::mem::offset_of!(#name, #ident),
                }
            });
            if let Some(range) = range {
                ranges.push(quote! { (#index, #range) });
            }
        }
    }

    // A resource is never inserted onto an entity, so it gets no `Bundle` impl —
    // `spawn(MyResource)` should not compile.
    let bundle = if is_resource {
        quote!()
    } else {
        quote! {
            impl ::renzora_plugin::ecs::Bundle for #name {
                fn write(self, e: &mut ::renzora_plugin::ecs::EntityCommands) {
                    e.insert_one(self);
                }
            }
        }
    };
    // `Component::descriptor` is optional because host-owned components like
    // `Transform` are looked up rather than registered. A resource is always
    // plugin-owned, so its descriptor is not.
    let (trait_path, desc_ret, desc_wrap) = if is_resource {
        (quote!(Resource), quote!(ComponentDesc), quote!(desc))
    } else {
        (quote!(Component), quote!(Option<ComponentDesc>), quote!(Some(desc)))
    };

    let expanded = quote! {
        const _: () = {
            use ::renzora_plugin::sys::{ComponentDesc, FieldDesc, FieldKind, StrRef};

            #bundle

            impl ::renzora_plugin::ecs::#trait_path for #name {
                // `module_path!()` makes the identity crate-qualified, so two
                // plugins can each define a `Spinner` without colliding. This
                // string is what scenes serialize — renaming the type or moving
                // it between modules breaks saved scenes, exactly as renaming a
                // Rust type would.
                const TYPE_PATH: &'static str =
                    ::core::concat!(::core::module_path!(), "::", ::core::stringify!(#name));

                fn display_name() -> &'static str { #display }

                fn id_cell() -> &'static ::core::sync::atomic::AtomicU32 {
                    static CELL: ::core::sync::atomic::AtomicU32 =
                        ::core::sync::atomic::AtomicU32::new(u32::MAX);
                    &CELL
                }

                fn fields() -> &'static [FieldDesc] {
                    static FIELDS: &[FieldDesc] = &[#(#entries),*];
                    FIELDS
                }

                fn field_ranges() -> &'static [(usize, ::renzora_plugin::sys::FieldRange)] {
                    static RANGES: &[(usize, ::renzora_plugin::sys::FieldRange)] =
                        &[#(#ranges),*];
                    RANGES
                }

                fn descriptor() -> #desc_ret {
                    // Writes a default-valued instance into host-provided
                    // storage. This is why the derive requires `Default`: the
                    // editor has to put *something* on the entity when you add
                    // the component, and zeroed memory is a bad answer — a
                    // `speed: 0.0` component is present, correct, and doing
                    // nothing, which reads as a broken plugin.
                    unsafe extern "C" fn init(out: *mut u8) {
                        out.cast::<#name>().write(<#name as ::core::default::Default>::default());
                    }
                    let desc = ComponentDesc {
                        name: StrRef::new(
                            <#name as ::renzora_plugin::ecs::#trait_path>::TYPE_PATH,
                        ),
                        size: ::core::mem::size_of::<#name>(),
                        align: ::core::mem::align_of::<#name>(),
                        drop: None,
                        display_name: StrRef::new(#display),
                        fields: <#name as ::renzora_plugin::ecs::#trait_path>::fields().as_ptr(),
                        field_count:
                            <#name as ::renzora_plugin::ecs::#trait_path>::fields().len(),
                        default_init: Some(init),
                    };
                    #desc_wrap
                }
            }
        };
    };
    expanded.into()
}
