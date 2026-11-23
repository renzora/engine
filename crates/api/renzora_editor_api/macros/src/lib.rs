mod field_parse;
mod inspectable;

use proc_macro::TokenStream;

/// Derive macro that generates an `InspectableComponent` impl for a struct.
///
/// # Attributes
/// - `#[inspectable(name = "...", icon = "...", category = "...")]` on the struct
/// - `#[field(speed = 0.01, min = 0.0, max = 1.0)]` on fields
/// - `#[field(skip)]` to exclude a field
/// - `#[field(readonly)]` for read-only display
/// - `#[field(name = "Display Name")]` to override the field label
#[proc_macro_derive(Inspectable, attributes(inspectable, field))]
pub fn derive_inspectable(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match inspectable::derive_inspectable(input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
