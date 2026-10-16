use convert_case::{Case, Casing};
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, FieldsNamed, Ident, Result};

pub fn storage_bindings(input: &DeriveInput) -> Result<quote::__private::TokenStream> {
    let name = &input.ident;

    let planar_name = Ident::new(&format!("Planar{name}"), name.span());
    let gpu_planar_name = Ident::new(&format!("PlanarStorage{name}"), name.span());
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
    let field_types = fields_struct.named.iter().map(|_| {
        quote! { bevy::render::render_resource::Buffer }
    });

    let bind_group = generate_bind_group_method(name, fields_struct);
    let bind_group_layout = generate_bind_group_layout_method(name, fields_struct);

    let buffers = field_names.clone().map(|name| {
        let buffer_name_string = format!("{name}_buffer");

        quote! {
            let #name = render_device.create_buffer_with_data(
                &bevy::render::render_resource::BufferInitDescriptor {
                    label: Some(#buffer_name_string),
                    contents: bytemuck::cast_slice(source.#name.as_slice()),
                    usage: bevy::render::render_resource::BufferUsages::COPY_DST
                         | bevy::render::render_resource::BufferUsages::STORAGE,
                }
            );
        }
    });

    let buffer_names = field_names.clone().map(|name| {
        quote! { #name }
    });

    let expanded = quote! {
        #[derive(Debug, Clone)]
        pub struct #gpu_planar_name {
            #(pub #field_names: #field_types,)*
            pub count: usize,
            pub draw_indirect_buffer: bevy::render::render_resource::Buffer,
        }

        impl bevy::render::render_asset::RenderAsset for #gpu_planar_name {
            type SourceAsset = #planar_name;
            type Param = bevy::ecs::system::lifetimeless::SRes<bevy::render::renderer::RenderDevice>;

            fn prepare_asset(
                source: Self::SourceAsset,
                _: AssetId<Self::SourceAsset>,
                render_device: &mut bevy::ecs::system::SystemParamItem<Self::Param>,
                _: Option<&Self>,
            ) -> Result<Self, bevy::render::render_asset::PrepareAssetError<Self::SourceAsset>> {
                let count = source.len();

                let draw_indirect_buffer = render_device.create_buffer_with_data(&bevy::render::render_resource::BufferInitDescriptor {
                    label: Some("draw indirect buffer"),
                    contents: wgpu::util::DrawIndirectArgs {  // TODO: reexport this type
                        vertex_count: 4,
                        instance_count: count as u32,
                        first_vertex: 0,
                        first_instance: 0,
                    }.as_bytes(),
                    usage: bevy::render::render_resource::BufferUsages::INDIRECT
                         | bevy::render::render_resource::BufferUsages::COPY_DST
                         | bevy::render::render_resource::BufferUsages::STORAGE
                         | bevy::render::render_resource::BufferUsages::COPY_SRC,
                });

                #(#buffers)*

                Ok(Self {
                    count,
                    draw_indirect_buffer,

                    #(#buffer_names),*
                })
            }

            fn asset_usage(_: &Self::SourceAsset) -> bevy::asset::RenderAssetUsages {
                bevy::asset::RenderAssetUsages::default()
            }
        }

        impl GpuPlanar for #gpu_planar_name {
            type PackedType = #name;
            type PlanarType = #planar_name;

            fn len(&self) -> usize {
                self.count
            }
        }

        impl GpuPlanarStorage for #gpu_planar_name {
            fn draw_indirect_buffer(&self) -> &bevy::render::render_resource::Buffer {
                return &self.draw_indirect_buffer;
            }

            #bind_group
            #bind_group_layout
        }

        impl PlanarSync for #name {
            type PackedType = #name;
            type PlanarType = #planar_name;
            type PlanarTypeHandle = #planar_handle_name;
            type GpuPlanarType = #gpu_planar_name;
        }
    };

    Ok(expanded)
}

pub fn generate_bind_group_method(
    struct_name: &Ident,
    fields_named: &FieldsNamed,
) -> quote::__private::TokenStream {
    let struct_name_snake = struct_name.to_string().to_case(Case::Snake);
    let bind_group_name = format!("storage_{struct_name_snake}_bind_group");

    let bind_group_entries = fields_named.named.iter().enumerate().map(|(idx, field)| {
        let name = field.ident.as_ref().unwrap();
        quote! {
            bevy::render::render_resource::BindGroupEntry {
                binding: #idx as u32,
                resource: bevy::render::render_resource::BindingResource::Buffer(
                    bevy::render::render_resource::BufferBinding {
                        buffer: &self.#name,
                        offset: 0,
                        size: bevy::render::render_resource::BufferSize::new(self.#name.size()),
                    }
                ),
            },
        }
    });

    quote! {
        fn bind_group(
            &self,
            render_device: &bevy::render::renderer::RenderDevice,
            layout: &bevy::render::render_resource::BindGroupLayout,
        ) -> bevy::render::render_resource::BindGroup {
            render_device.create_bind_group(
                #bind_group_name,
                &layout,
                &[
                    #(#bind_group_entries)*
                ]
            )
        }
    }
}

pub fn generate_bind_group_layout_method(
    struct_name: &Ident,
    fields_named: &FieldsNamed,
) -> quote::__private::TokenStream {
    let struct_name_snake = struct_name.to_string().to_case(Case::Snake);
    let bind_group_layout_name = format!("storage_{struct_name_snake}_bind_group_layout");

    let bind_group_layout_entries = fields_named.named
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            quote! {
                bevy::render::render_resource::BindGroupLayoutEntry {
                    binding: #idx as u32,
                    visibility: bevy::render::render_resource::ShaderStages::VERTEX_FRAGMENT
                        | bevy::render::render_resource::ShaderStages::COMPUTE,
                    ty: bevy::render::render_resource::BindingType::Buffer {
                        ty: bevy::render::render_resource::BufferBindingType::Storage { read_only },
                        has_dynamic_offset: false,
                        min_binding_size: bevy::render::render_resource::BufferSize::new(Self::PackedType::min_binding_sizes()[#idx] as u64),
                    },
                    count: None,
                },
            }
        });

    quote! {
        fn bind_group_layout(
            render_device: &bevy::render::renderer::RenderDevice,
            read_only: bool,
        ) -> bevy::render::render_resource::BindGroupLayout {
            render_device.create_bind_group_layout(
                Some(#bind_group_layout_name),
                &[
                    #(#bind_group_layout_entries)*
                ],
            )
        }
    }
}
