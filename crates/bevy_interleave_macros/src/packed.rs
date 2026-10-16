use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, FieldsNamed, Result};

pub fn generate_reflect_interleaved(input: &DeriveInput) -> Result<quote::__private::TokenStream> {
    let name = &input.ident;

    let fields_struct = if let Data::Struct(ref data_struct) = input.data {
        match data_struct.fields {
            Fields::Named(ref fields) => fields,
            _ => return Err(Error::new_spanned(input, "Unsupported struct type")),
        }
    } else {
        return Err(Error::new_spanned(
            input,
            "Planar macro only supports structs",
        ));
    };

    let min_binding_size_method = generate_min_binding_size_method(fields_struct);
    let ordered_field_names_method = generate_ordered_field_names_method(fields_struct);

    let expanded = quote! {
        impl ReflectInterleaved for #name {
            type PackedType = #name;

            #min_binding_size_method
            #ordered_field_names_method
        }
    };

    Ok(expanded)
}

pub fn generate_min_binding_size_method(
    fields_named: &FieldsNamed,
) -> quote::__private::TokenStream {
    let min_binding_sizes = fields_named.named.iter().map(|f| {
        let field_type = &f.ty;
        quote! {
            std::mem::size_of::<#field_type>()
        }
    });

    quote! {
        fn min_binding_sizes() -> &'static [usize] {
            &[#(#min_binding_sizes),*]
        }
    }
}

pub fn generate_ordered_field_names_method(
    fields_named: &FieldsNamed,
) -> quote::__private::TokenStream {
    let string_field_names = fields_named.named.iter().map(|field| {
        let name = field.ident.as_ref().unwrap();
        let name_str = name.to_string();
        quote! { #name_str }
    });

    quote! {
        fn ordered_field_names() -> &'static [&'static str] {
            &[
                #(#string_field_names),*
            ]
        }
    }
}
