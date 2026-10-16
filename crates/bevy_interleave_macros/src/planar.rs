use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, FieldsNamed, Ident, Result};

pub fn generate_planar_struct(input: &DeriveInput) -> Result<quote::__private::TokenStream> {
    let name = &input.ident;
    let planar_name = Ident::new(&format!("Planar{name}"), name.span());
    let planar_handle_name = Ident::new(&format!("Planar{name}Handle"), name.span());

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

    let field_names = fields_struct
        .named
        .iter()
        .map(|f| f.ident.as_ref().unwrap());
    let field_types = fields_struct.named.iter().map(|og| {
        let ty = &og.ty;
        quote! { Vec<#ty> }
    });

    let conversion_methods = generate_conversion_methods(name, fields_struct);
    let get_set_methods = generate_accessor_setter_methods(name, fields_struct);
    let len_method = generate_len_method(fields_struct);
    let subset_method = generate_subset_method(fields_struct);

    let expanded = quote! {
        #[derive(
            bevy::asset::Asset,
            Clone,
            Debug,
            Default,
            PartialEq,
            bevy::reflect::Reflect,
            serde::Serialize,
            serde::Deserialize,
        )]
        pub struct #planar_name {
            #(pub #field_names: #field_types,)*
        }

        impl Planar for #planar_name {
            type PackedType = #name;

            #conversion_methods
            #get_set_methods
            #len_method
            #subset_method
        }

        #[derive(bevy::prelude::Component, Clone, Debug, Default, PartialEq, bevy::reflect::Reflect)]
        #[require(bevy::prelude::Transform, bevy::prelude::Visibility)]
        pub struct #planar_handle_name(pub bevy::asset::Handle<#planar_name>);

        impl bevy_interleave::interface::PlanarHandle<#planar_name> for #planar_handle_name {
            fn handle(&self) -> &bevy::asset::Handle<#planar_name> {
                &self.0
            }
        }

        impl bevy::render::sync_component::SyncComponent<bevy::render::RenderApp> for #planar_handle_name {
            type Target = Self;
        }
    };

    Ok(expanded)
}

pub fn generate_len_method(fields_named: &FieldsNamed) -> quote::__private::TokenStream {
    if let Some(first_field) = fields_named.named.first() {
        let first_field_name = first_field.ident.as_ref().unwrap();
        quote! {
            fn is_empty(&self) -> bool {
                self.#first_field_name.is_empty()
            }

            fn len(&self) -> usize {
                self.#first_field_name.len()
            }
        }
    } else {
        quote! {
            fn is_empty(&self) -> bool {
                true
            }

            fn len(&self) -> usize {
                0
            }
        }
    }
}

pub fn generate_accessor_setter_methods(
    struct_name: &Ident,
    fields_named: &FieldsNamed,
) -> quote::__private::TokenStream {
    let packed_assignments = fields_named.named.iter().map(|field| {
        let name = field.ident.as_ref().unwrap();
        quote! { #name: self.#name[index].clone() }
    });

    let set_assignments = fields_named.named.iter().map(|field| {
        let name = field.ident.as_ref().unwrap();
        quote! { self.#name[index] = value.#name.clone(); }
    });

    quote! {
        fn get(&self, index: usize) -> #struct_name {
            #struct_name {
                #(#packed_assignments),*
            }
        }

        fn set(&mut self, index: usize, value: #struct_name) {
            #(#set_assignments);*
        }
    }
}

pub fn generate_conversion_methods(
    struct_name: &Ident,
    fields_named: &FieldsNamed,
) -> quote::__private::TokenStream {
    let (from_interleaved_fields, to_interleaved_fields_templates): (Vec<_>, Vec<_>) = fields_named
        .named
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();

            let from_interleaved_field = quote! {
                #name: packed.iter().map(|x| x.#name.clone()).collect()
            };
            let to_interleaved_field_template = quote! {
                #name: self.#name[index].clone()
            };

            (from_interleaved_field, to_interleaved_field_template)
        })
        .unzip();

    let to_interleaved_method = quote! {
        fn to_interleaved(&self) -> Vec<#struct_name> {
            (0..self.len())
                .map(|index| #struct_name {
                    #(#to_interleaved_fields_templates),*
                })
                .collect()
        }
    };

    let conversion_methods = quote! {
        fn from_interleaved(packed: Vec<#struct_name>) -> Self {
            Self {
                #(#from_interleaved_fields),*
            }
        }
        #to_interleaved_method
    };

    conversion_methods
}

pub fn generate_subset_method(fields_named: &FieldsNamed) -> proc_macro2::TokenStream {
    let mut new_planes_fields = Vec::new();
    let mut push_self_index = Vec::new();
    let mut planes = Vec::new();

    for field in &fields_named.named {
        let name = field.ident.as_ref().unwrap();

        new_planes_fields.push(quote! {
            let mut #name = Vec::with_capacity(indices.len());
        });
        push_self_index.push(quote! {
            #name.push(self.#name[index]);
        });
        planes.push(quote! {
            #name
        });
    }

    quote! {
        fn subset(&self, indices: &[usize]) -> Self {
            #(#new_planes_fields)*

            for &index in indices {
                #(#push_self_index)*
            }

            Self {
                #(#planes),*
            }
        }
    }
}
