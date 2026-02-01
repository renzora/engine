use bevy::{
    ecs::{
        query::ROQueryItem,
        system::{
            lifetimeless::{Read, SRes},
            SystemParamItem,
        },
    },
    pbr::{DrawMesh, SetMeshBindGroup},
    render::{
        extract_component::DynamicUniformIndex,
        render_phase::{
            PhaseItem, PhaseItemExtraIndex, RenderCommand, RenderCommandResult, SetItemPipeline,
            TrackedRenderPass,
        },
    },
};

use crate::{
    uniforms::{AlphaMaskBindGroups, OutlineInstanceBindGroup, RenderOutlineInstances},
    view_uniforms::{OutlineViewBindGroup, OutlineViewUniform},
};

pub(crate) struct SetOutlineInstanceBindGroup<const I: usize>();

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetOutlineInstanceBindGroup<I> {
    type ViewQuery = ();
    type ItemQuery = ();
    type Param = SRes<OutlineInstanceBindGroup>;
    fn render<'w>(
        item: &P,
        _view_data: (),
        _entity_data: Option<()>,
        bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let dynamic_uniform_index = match item.extra_index() {
            PhaseItemExtraIndex::DynamicOffset(index) => Some(index),
            _ => None,
        };
        pass.set_bind_group(
            I,
            &bind_group.into_inner().bind_group,
            dynamic_uniform_index.as_slice(),
        );
        RenderCommandResult::Success
    }
}

pub(crate) struct SetOutlineViewBindGroup<const I: usize>();

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetOutlineViewBindGroup<I> {
    type ViewQuery = Read<DynamicUniformIndex<OutlineViewUniform>>;
    type ItemQuery = ();
    type Param = SRes<OutlineViewBindGroup>;
    fn render<'w>(
        _item: &P,
        view_data: ROQueryItem<'w, 'w, Self::ViewQuery>,
        _entity_data: Option<()>,
        bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_bind_group(I, &bind_group.into_inner().bind_group, &[view_data.index()]);
        RenderCommandResult::Success
    }
}

pub(crate) struct SetOutlineAlphaMaskBindGroup<const I: usize>();

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetOutlineAlphaMaskBindGroup<I> {
    type ViewQuery = ();
    type ItemQuery = ();
    type Param = (SRes<AlphaMaskBindGroups>, SRes<RenderOutlineInstances>);
    fn render<'w>(
        item: &P,
        _view_data: (),
        _entity_data: Option<ROQueryItem<'w, 'w, Self::ItemQuery>>,
        (bind_groups, render_outlines): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(outline) = render_outlines.get(&item.main_entity()) else {
            return RenderCommandResult::Failure("No outline found for entity.");
        };
        let bind_groups = bind_groups.into_inner();
        let bind_group = if let Some(texture_handle) = outline.alpha_mask_id {
            bind_groups
                .bind_groups
                .get(&texture_handle)
                .unwrap_or(&bind_groups.default_bind_group)
        } else {
            &bind_groups.default_bind_group
        };

        pass.set_bind_group(I, bind_group, &[]);
        RenderCommandResult::Success
    }
}

pub(crate) type DrawOutline = (
    SetItemPipeline,
    SetOutlineViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetOutlineInstanceBindGroup<2>,
    SetOutlineAlphaMaskBindGroup<3>,
    DrawMesh,
);
