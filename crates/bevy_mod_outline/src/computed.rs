use bevy::{camera::visibility::RenderLayers, ecs::query::QueryItem, prelude::*};

use crate::{
    pipeline_key::ComputedOutlineKey,
    uniforms::{DepthMode, DrawMode},
    InheritOutline, OutlineAlphaMask, OutlineMode, OutlinePlaneDepth, OutlineRenderLayers,
    OutlineStencil, OutlineVolume, OutlineWarmUp,
};

#[derive(Clone)]
pub(crate) struct ComputedVolume {
    pub(crate) enabled: bool,
    pub(crate) offset: f32,
    pub(crate) colour: LinearRgba,
}

#[derive(Clone)]
pub(crate) struct ComputedStencil {
    pub(crate) enabled: bool,
    pub(crate) offset: f32,
}

#[derive(Clone)]
pub(crate) struct ComputedMode {
    pub(crate) depth_mode: DepthMode,
    pub(crate) draw_mode: DrawMode,
    pub(crate) double_sided: bool,
}

#[derive(Clone)]
pub(crate) struct ComputedDepth {
    pub(crate) world_plane_origin: Vec3,
    pub(crate) world_plane_offset: Vec3,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Source {
    Set,
    SetFallback,
    Inherited,
    Default,
}

#[derive(Clone)]
pub(crate) struct Sourced<T: Clone> {
    pub(crate) value: T,
    pub(crate) source: Source,
}

impl<T: Clone> Sourced<T> {
    pub fn set<U: Clone + Default>(
        value: Option<Ref<U>>,
        inherit: Option<T>,
        f: impl FnOnce(&U) -> T,
    ) -> Self {
        Self::set_with_fallback::<U, U>(value, None, inherit, f)
    }

    pub fn set_with_fallback<U: Default, V: Clone + Into<U>>(
        value: Option<Ref<U>>,
        fallback: Option<Ref<V>>,
        inherit: Option<T>,
        f: impl FnOnce(&U) -> T,
    ) -> Self {
        if let Some(v) = value {
            Sourced {
                value: f(&v),
                source: Source::Set,
            }
        } else if let Some(v) = fallback {
            Sourced {
                value: f(&v.clone().into()),
                source: Source::SetFallback,
            }
        } else if let Some(v) = inherit {
            Sourced {
                value: v,
                source: Source::Inherited,
            }
        } else {
            Sourced {
                value: f(&U::default()),
                source: Source::Default,
            }
        }
    }

    pub fn is_changed<U>(&self, value: &Option<Ref<U>>, inherit: bool) -> bool {
        self.is_changed_with_fallback::<U, U>(value, &None, inherit)
    }

    pub fn is_changed_with_fallback<U, V>(
        &self,
        value: &Option<Ref<U>>,
        fallback: &Option<Ref<V>>,
        inherit: bool,
    ) -> bool {
        let (source, changed) = if let Some(r) = value {
            (Source::Set, r.is_changed())
        } else if let Some(r) = fallback {
            (Source::SetFallback, r.is_changed())
        } else if inherit {
            (Source::Inherited, false)
        } else {
            (Source::Default, false)
        };
        source != self.source || changed
    }
}

#[derive(Clone)]
pub(crate) struct ComputedInternal {
    pub(crate) inherited_from: Option<Entity>,
    pub(crate) volume: Sourced<ComputedVolume>,
    pub(crate) stencil: Sourced<ComputedStencil>,
    pub(crate) mode: Sourced<ComputedMode>,
    pub(crate) depth: Sourced<ComputedDepth>,
    pub(crate) layers: Sourced<RenderLayers>,
    pub(crate) alpha_mask: Sourced<OutlineAlphaMask>,
    pub(crate) warm_up: Sourced<OutlineWarmUp>,
}

/// A component for storing the computed depth at which the outline lies.
#[derive(Clone, Component, Default)]
#[require(ComputedOutlineKey)]
pub struct ComputedOutline(pub(crate) Option<ComputedInternal>);

type OutlineComponents<'a> = (
    Ref<'a, InheritedVisibility>,
    Ref<'a, GlobalTransform>,
    Option<Ref<'a, OutlineVolume>>,
    Option<Ref<'a, OutlineStencil>>,
    Option<Ref<'a, OutlineMode>>,
    Option<Ref<'a, OutlinePlaneDepth>>,
    Option<Ref<'a, OutlineRenderLayers>>,
    Option<Ref<'a, RenderLayers>>,
    Option<Ref<'a, OutlineAlphaMask>>,
    Option<Ref<'a, OutlineWarmUp>>,
);

#[allow(clippy::type_complexity)]
pub(crate) fn compute_outline(
    mut root_query: Query<
        (
            Entity,
            &mut ComputedOutline,
            OutlineComponents,
            Option<&Children>,
        ),
        Without<InheritOutline>,
    >,
    mut child_query_mut: Query<(&mut ComputedOutline, OutlineComponents), With<InheritOutline>>,
    child_query: Query<&Children>,
) {
    for (entity, mut computed, components, children) in root_query.iter_mut() {
        let changed = update_computed_outline(&mut computed, components, None, None, false);
        if let Some(cs) = children {
            let parent_computed = computed.0.as_ref().unwrap();
            for child in cs.iter() {
                propagate_computed_outline(
                    parent_computed,
                    changed,
                    entity,
                    child,
                    &mut child_query_mut,
                    &child_query,
                );
            }
        }
    }
}

fn propagate_computed_outline(
    parent_computed: &ComputedInternal,
    parent_changed: bool,
    parent_entity: Entity,
    entity: Entity,
    child_query_mut: &mut Query<(&mut ComputedOutline, OutlineComponents), With<InheritOutline>>,
    child_query: &Query<&Children>,
) {
    if let Ok((mut computed, components)) = child_query_mut.get_mut(entity) {
        let changed = update_computed_outline(
            &mut computed,
            components,
            Some(parent_computed),
            Some(parent_entity),
            parent_changed,
        );
        if let Ok(cs) = child_query.get(entity) {
            let parent_computed = &computed.0.as_ref().unwrap().clone();
            for child in cs.iter() {
                propagate_computed_outline(
                    parent_computed,
                    changed,
                    entity,
                    child,
                    child_query_mut,
                    child_query,
                );
            }
        }
    }
}

fn update_computed_outline(
    computed: &mut Mut<'_, ComputedOutline>,
    (
        visibility,
        transform,
        volume,
        stencil,
        mode,
        depth,
        layers,
        fallback_layers,
        alpha_mask,
        warm_up,
    ): QueryItem<'_, '_, OutlineComponents>,
    parent_computed: Option<&ComputedInternal>,
    parent_entity: Option<Entity>,
    force_update: bool,
) -> bool {
    let has_parent = parent_computed.is_some();
    let changed = force_update
        || if let ComputedOutline(Some(computed)) = computed.as_ref() {
            computed.inherited_from != parent_entity
                || visibility.is_changed()
                || transform.is_changed()
                || computed.volume.is_changed(&volume, has_parent)
                || computed.stencil.is_changed(&stencil, has_parent)
                || computed.mode.is_changed(&mode, has_parent)
                || computed.depth.is_changed(&depth, has_parent)
                || computed
                    .layers
                    .is_changed_with_fallback(&layers, &fallback_layers, has_parent)
                || computed.alpha_mask.is_changed(&alpha_mask, has_parent)
        } else {
            true
        };
    if changed {
        computed.0 = Some(ComputedInternal {
            inherited_from: parent_entity,
            volume: Sourced::set(
                volume,
                parent_computed.map(|p| p.volume.value.clone()),
                |vol| ComputedVolume {
                    enabled: visibility.get() && vol.visible && !vol.colour.is_fully_transparent(),
                    offset: vol.width,
                    colour: vol.colour.into(),
                },
            ),
            stencil: Sourced::set(
                stencil,
                parent_computed.map(|p| p.stencil.value.clone()),
                |sten| ComputedStencil {
                    enabled: visibility.get() && sten.enabled,
                    offset: sten.offset,
                },
            ),
            mode: Sourced::set(
                mode,
                parent_computed.map(|p| p.mode.value.clone()),
                |mode| match mode {
                    OutlineMode::ExtrudeFlat => ComputedMode {
                        depth_mode: DepthMode::Flat,
                        draw_mode: DrawMode::Extrude,
                        double_sided: false,
                    },
                    OutlineMode::ExtrudeFlatDoubleSided => ComputedMode {
                        depth_mode: DepthMode::Flat,
                        draw_mode: DrawMode::Extrude,
                        double_sided: true,
                    },
                    OutlineMode::ExtrudeReal => ComputedMode {
                        depth_mode: DepthMode::Real,
                        draw_mode: DrawMode::Extrude,
                        double_sided: false,
                    },
                    #[cfg(feature = "flood")]
                    OutlineMode::FloodFlat => ComputedMode {
                        depth_mode: DepthMode::Flat,
                        draw_mode: DrawMode::JumpFlood,
                        double_sided: false,
                    },
                    #[cfg(feature = "flood")]
                    OutlineMode::FloodFlatDoubleSided => ComputedMode {
                        depth_mode: DepthMode::Flat,
                        draw_mode: DrawMode::JumpFlood,
                        double_sided: true,
                    },
                },
            ),
            depth: Sourced::set(
                depth,
                parent_computed.map(|p| p.depth.value.clone()),
                |dep| {
                    let affine = transform.affine();
                    let inverse = transform.affine().matrix3.inverse();
                    ComputedDepth {
                        world_plane_origin: (affine
                            .matrix3
                            .mul_vec3a(dep.model_plane_origin.into())
                            + affine.translation)
                            .into(),
                        world_plane_offset: inverse.mul_vec3(dep.model_plane_offset),
                    }
                },
            ),
            layers: Sourced::set_with_fallback(
                layers,
                fallback_layers,
                parent_computed.map(|p| p.layers.value.clone()),
                |layers| layers.0.clone(),
            ),
            alpha_mask: Sourced::set(
                alpha_mask,
                parent_computed.map(|p| p.alpha_mask.value.clone()),
                |mask| mask.clone(),
            ),
            warm_up: Sourced::set(
                warm_up,
                parent_computed.map(|p| p.warm_up.value.clone()),
                |warm_up| warm_up.clone(),
            ),
        });
    }
    changed
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    fn setup() -> (App, Entity) {
        let mut app = App::new();
        app.add_systems(Update, compute_outline);
        let entity = app
            .world_mut()
            .spawn((
                ComputedOutline::default(),
                InheritedVisibility::VISIBLE,
                GlobalTransform::default(),
            ))
            .id();
        (app, entity)
    }

    #[test]
    fn test_defaults() {
        let (mut app, entity) = setup();
        app.update();

        let computed = app
            .world()
            .get::<ComputedOutline>(entity)
            .expect("Entity should have ComputedOutline");
        let internal = computed
            .0
            .as_ref()
            .expect("ComputedOutline should have Some value after update");
        assert!(internal.stencil.value.enabled);
        assert!(!internal.volume.value.enabled);

        // Update the system again and check that nothing has changed
        let tick = app
            .world_mut()
            .run_system_once(|query: Query<Ref<ComputedOutline>>| {
                query.single().unwrap().last_changed()
            })
            .unwrap();
        app.update();
        app.world_mut()
            .run_system_once(move |query: Query<Ref<ComputedOutline>>| {
                assert_eq!(query.single().unwrap().last_changed(), tick);
            })
            .unwrap();
    }

    #[test]
    fn test_volume_propagation() {
        let (mut app, parent) = setup();

        // Create a child entity that inherits outline properties
        let child = app
            .world_mut()
            .spawn((
                InheritOutline,
                ComputedOutline::default(),
                InheritedVisibility::VISIBLE,
                GlobalTransform::default(),
            ))
            .insert(ChildOf(parent))
            .id();

        // Add an OutlineVolume to the parent
        let volume = OutlineVolume {
            visible: true,
            width: 2.0,
            colour: Color::WHITE,
        };
        app.world_mut().entity_mut(parent).insert(volume.clone());

        // Update the system
        app.update();

        // Check parent computed outline
        let parent_computed = app
            .world()
            .get::<ComputedOutline>(parent)
            .expect("Parent entity should have ComputedOutline component");
        let parent_internal = parent_computed
            .0
            .as_ref()
            .expect("Parent ComputedOutline should have Some value after update");
        assert!(parent_internal.volume.value.enabled);
        assert_eq!(parent_internal.volume.value.offset, 2.0);
        assert_eq!(parent_internal.volume.source, Source::Set);
        assert_eq!(parent_internal.inherited_from, None);

        // Check child computed outline
        let child_computed = app
            .world()
            .get::<ComputedOutline>(child)
            .expect("Child entity should have ComputedOutline component");
        let child_internal = child_computed
            .0
            .as_ref()
            .expect("Child ComputedOutline should have Some value after update");
        assert!(child_internal.volume.value.enabled);
        assert_eq!(child_internal.volume.value.offset, 2.0);
        assert_eq!(child_internal.volume.source, Source::Inherited);
        assert_eq!(child_internal.inherited_from, Some(parent));
    }
}
