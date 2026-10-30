//! [`Transform`] extrapolation, making movement in [`FixedUpdate`] appear smooth
//! by easing between the current and predicted [`Transform`] in between fixed timesteps.
//!
//! See the [`TransformExtrapolationPlugin`] for more information.

use core::marker::PhantomData;

use crate::{
    NoRotationEasing, NoTranslationEasing, RotationEasingState, TransformEasingPlugin,
    TransformEasingSystems, TranslationEasingState, VelocitySource, VelocitySourceItem,
};
use bevy::prelude::*;

/// A plugin for [`Transform`] extrapolation, making movement in [`FixedUpdate`] appear smooth.
///
/// Transform extrapolation predicts future positions based on velocity, and applies easing
/// between the current and predicted [`Transform`] in between fixed timesteps.
/// This results in movement that looks smooth and feels responsive, but can stutter
/// when the prediction is incorrect, such as when velocity changes abruptly.
///
/// This plugin requires the [`TransformEasingPlugin`] to function. It is automatically added
/// if not already present in the app.
///
/// Note that unlike [`TransformInterpolationPlugin`], this plugin does *not* support scale easing.
/// However, the [`ScaleInterpolation`] component can still be used even when translation and rotation are extrapolated.
///
/// [`TransformInterpolationPlugin`]: crate::interpolation::TransformInterpolationPlugin
/// [`ScaleInterpolation`]: crate::interpolation::ScaleInterpolation
///
/// # Usage
///
/// Transform extrapolation requires velocity to predict future positions.
/// Instead of providing its own velocity components, the [`TransformExtrapolationPlugin`]
/// lets you specify your own velocity components that you manage yourself.
///
/// First, make sure you have components for velocity, and implement the [`VelocitySource`] trait on a [`QueryData`] type:
///
/// ```
/// use bevy::{ecs::query::QueryData, prelude::*};
/// use bevy_transform_interpolation::VelocitySource;
///
/// #[derive(Component, Default)]
/// struct LinearVelocity(Vec3);
///
/// #[derive(Component, Default)]
/// struct AngularVelocity(Vec3);
///
/// #[derive(QueryData)]
/// struct LinVelSource;
///
/// impl VelocitySource for LinVelSource {
///     // Components storing the previous and current velocities.
///     // Note: For extrapolation, the `Previous` component is not used, so we can make it the same as `Current`.
///     type Previous = LinearVelocity;
///     type Current = LinearVelocity;
///
///     fn previous(start: &Self::Previous) -> Vec3 {
///         start.0
///     }
///
///     fn current(end: &Self::Current) -> Vec3 {
///         end.0
///     }
/// }
///
/// #[derive(QueryData)]
/// struct AngVelSource;
///
/// impl VelocitySource for AngVelSource {
///     type Previous = AngularVelocity;
///     type Current = AngularVelocity;
///
///     fn previous(start: &Self::Previous) -> Vec3 {
///         start.0
///     }
///
///     fn current(end: &Self::Current) -> Vec3 {
///         end.0
///     }
/// }
/// ```
///
/// Then, add the [`TransformExtrapolationPlugin`] to the app with the velocity sources:
///
/// ```no_run
/// use bevy::{ecs::query::QueryData, prelude::*};
/// use bevy_transform_interpolation::{prelude::*, VelocitySource};
/// #
/// # #[derive(Component, Default)]
/// # struct LinearVelocity(Vec3);
/// #
/// # #[derive(Component, Default)]
/// # struct AngularVelocity(Vec3);
/// #
/// # #[derive(QueryData)]
/// # struct LinVelSource;
/// #
/// # impl VelocitySource for LinVelSource {
/// #     type Previous = LinearVelocity;
/// #     type Current = LinearVelocity;
/// #
/// #     fn previous(start: &Self::Previous) -> Vec3 {
/// #         start.0
/// #     }
/// #
/// #     fn current(end: &Self::Current) -> Vec3 {
/// #         end.0
/// #     }
/// # }
/// #
/// # #[derive(QueryData)]
/// # struct AngVelSource;
/// #
/// # impl VelocitySource for AngVelSource {
/// #     type Previous = AngularVelocity;
/// #     type Current = AngularVelocity;
/// #
/// #     fn previous(start: &Self::Previous) -> Vec3 {
/// #         start.0
/// #     }
/// #
/// #     fn current(end: &Self::Current) -> Vec3 {
/// #         end.0
/// #     }
/// # }
///
/// fn main() {
///    let mut app = App::new();
///
///     app.add_plugins((
///        TransformInterpolationPlugin::default(),
///        TransformExtrapolationPlugin::<LinVelSource, AngVelSource>::default(),
/// #      bevy::time::TimePlugin::default(),
///    ));
///
///    // Optional: Insert velocity components automatically for entities with extrapolation.
///    app.register_required_components::<TranslationExtrapolation, LinearVelocity>();
///    app.register_required_components::<RotationExtrapolation, AngularVelocity>();
///
///    // ...
///
///    app.run();
/// }
/// ```
///
/// Transform extrapolation can now be enabled for a given entity by adding the [`TransformExtrapolation`] component:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_transform_interpolation::prelude::*;
/// #
/// fn setup(mut commands: Commands) {
///     // Extrapolate translation and rotation.
///     commands.spawn((
///         Transform::default(),
///         TransformExtrapolation,
///     ));
/// }
/// ```
///
/// Now, any changes made to the translation or rotation of the entity in [`FixedPreUpdate`], [`FixedUpdate`],
/// or [`FixedPostUpdate`] will automatically be smoothed in between fixed timesteps.
///
/// Transform properties can also be extrapolated individually by adding the [`TranslationExtrapolation`]
/// and [`RotationExtrapolation`] components.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_transform_interpolation::prelude::*;
/// #
/// fn setup(mut commands: Commands) {
///     // Only extrapolate translation.
///     commands.spawn((Transform::default(), TranslationExtrapolation));
///     
///     // Only extrapolate rotation.
///     commands.spawn((Transform::default(), RotationExtrapolation));
/// }
/// ```
///
/// If you want *all* entities with a [`Transform`] to be extrapolated by default, you can use
/// [`TransformExtrapolationPlugin::extrapolate_all()`], or set the [`extrapolate_translation_all`]
/// and [`extrapolate_rotation_all`] fields.
///
/// ```ignore
/// # use bevy::prelude::*;
/// # use bevy_transform_interpolation::prelude::*;
/// #
/// fn main() {
///    App::new()
///       .add_plugins(TransformExtrapolationPlugin::<LinVelSource, AngVelSource> {
///           // Extrapolate translation by default, but not rotation.
///           extrapolate_translation_all: true,
///           extrapolate_rotation_all: false,
///       })
///       // ...
///       .run();
/// }
/// ```
///
/// When extrapolation is enabled for all entities by default, you can still opt out of it for individual entities
/// by adding the [`NoTransformEasing`] component, or the individual [`NoTranslationEasing`] and [`NoRotationEasing`] components.
///
/// Note that changing [`Transform`] manually in any schedule that *doesn't* use a fixed timestep is also supported,
/// but it is equivalent to teleporting, and disables extrapolation for the entity for the remainder of that fixed timestep.
///
/// [`QueryData`]: bevy::ecs::query::QueryData
/// [`TransformExtrapolationPlugin::extrapolate_all()`]: TransformExtrapolationPlugin::extrapolate_all
/// [`extrapolate_translation_all`]: TransformExtrapolationPlugin::extrapolate_translation_all
/// [`extrapolate_rotation_all`]: TransformExtrapolationPlugin::extrapolate_rotation_all
/// [`NoTransformEasing`]: crate::NoTransformEasing
/// [`NoTranslationEasing`]: crate::NoTranslationEasing
/// [`NoRotationEasing`]: crate::NoRotationEasing
///
/// # Alternatives
///
/// For many applications, the stutter caused by mispredictions in extrapolation may be undesirable.
/// In these cases, the [`TransformInterpolationPlugin`] can be a better alternative.
///
/// Transform interpolation eases between the previous and current [`Transform`],
/// resulting in movement that is always smooth and accurate. The downside is that the rendered
/// positions can lag slightly behind the true positions, making movement feel delayed.
///
/// # Easing Backends
///
/// By default, transform extrapolation uses linear interpolation (`lerp`) for easing translation,
/// and spherical linear interpolation (`slerp`) for easing rotation.
///
/// If the previous and current velocities are also available, it is possible to use *Hermite interpolation*
/// with the [`TransformHermiteEasingPlugin`] to get smoother and more accurate easing. To enable Hermite interpolation
/// for extrapolation, add the [`TransformHermiteEasing`] component to the entity in addition to the extrapolation components.
///
/// [`TransformHermiteEasingPlugin`]: crate::hermite::TransformHermiteEasingPlugin
/// [`TransformHermiteEasing`]: crate::hermite::TransformHermiteEasing
#[derive(Debug)]
pub struct TransformExtrapolationPlugin<LinVel: VelocitySource, AngVel: VelocitySource> {
    /// If `true`, translation will be extrapolated for all entities with the [`Transform`] component by default.
    ///
    /// This can be overridden for individual entities by adding the [`NoTranslationEasing`] or [`NoTransformEasing`] component.
    ///
    /// [`NoTransformEasing`]: crate::NoTransformEasing
    pub extrapolate_translation_all: bool,
    /// If `true`, rotation will be extrapolated for all entities with the [`Transform`] component by default.
    ///
    /// This can be overridden for individual entities by adding the [`NoRotationEasing`] or [`NoTransformEasing`] component.
    ///
    /// [`NoTransformEasing`]: crate::NoTransformEasing
    pub extrapolate_rotation_all: bool,
    /// Phantom data use the type parameters.
    #[doc(hidden)]
    pub _phantom: PhantomData<(LinVel, AngVel)>,
}

impl<LinVel: VelocitySource, AngVel: VelocitySource> Default
    for TransformExtrapolationPlugin<LinVel, AngVel>
{
    fn default() -> Self {
        Self {
            extrapolate_translation_all: false,
            extrapolate_rotation_all: false,
            _phantom: PhantomData,
        }
    }
}

impl<LinVel: VelocitySource, AngVel: VelocitySource> TransformExtrapolationPlugin<LinVel, AngVel> {
    /// Enables extrapolation for translation and rotation for all entities with the [`Transform`] component.
    ///
    /// This can be overridden for individual entities by adding the [`NoTransformEasing`] component,
    /// or the individual [`NoTranslationEasing`] and [`NoRotationEasing`] components.
    ///
    /// [`NoTransformEasing`]: crate::NoTransformEasing
    /// [`NoRotationEasing`]: crate::NoRotationEasing
    pub fn extrapolate_all() -> Self {
        Self {
            extrapolate_translation_all: true,
            extrapolate_rotation_all: true,
            _phantom: PhantomData,
        }
    }
}

impl<LinVel: VelocitySource, AngVel: VelocitySource> Plugin
    for TransformExtrapolationPlugin<LinVel, AngVel>
{
    fn build(&self, app: &mut App) {
        //Register components.
        app.register_type::<(
            TransformExtrapolation,
            TranslationExtrapolation,
            RotationExtrapolation,
        )>();

        // Reset the transform to the start of the extrapolation at the beginning of the fixed timestep
        // to match the true position from the end of the previous fixed tick.
        app.add_systems(
            FixedFirst,
            (
                reset_translation_extrapolation,
                reset_rotation_extrapolation,
            )
                .before(TransformEasingSystems::Reset),
        );

        // Update the start and end state of the extrapolation at the end of the fixed timestep.
        app.add_systems(
            FixedLast,
            (
                update_translation_extrapolation_states::<LinVel>,
                update_rotation_extrapolation_states::<AngVel>,
            )
                .in_set(TransformEasingSystems::UpdateEnd),
        );

        // Insert extrapolation components automatically for all entities with a `Transform`
        // if the corresponding global extrapolation is enabled.
        if self.extrapolate_translation_all {
            let _ = app.try_register_required_components::<Transform, TranslationExtrapolation>();
        }
        if self.extrapolate_rotation_all {
            let _ = app.try_register_required_components::<Transform, RotationExtrapolation>();
        }
    }

    fn finish(&self, app: &mut App) {
        // Add the `TransformEasingPlugin` if it hasn't been added yet.
        // It performs the actual easing based on the start and end states set by the extrapolation.
        if !app.is_plugin_added::<TransformEasingPlugin>() {
            app.add_plugins(TransformEasingPlugin);
        }
    }
}

/// Enables [`Transform`] extrapolation for an entity, making changes to translation
/// and rotation in [`FixedUpdate`] appear smooth.
///
/// Extrapolation only works for entities with velocity components.
/// [`TransformExtrapolationPlugin`] must be added to the app with the appropriate velocity sources.
///
/// Unlike [`TransformInterpolation`], this does *not* support scale easing.
/// However, the [`ScaleInterpolation`] component can still be used even when translation and rotation are extrapolated.
///
/// See the [`TransformExtrapolationPlugin`] for more information.
///
/// [`TransformInterpolation`]: crate::interpolation::TransformInterpolation
/// [`ScaleInterpolation`]: crate::interpolation::ScaleInterpolation
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
#[require(TranslationExtrapolation, RotationExtrapolation)]
pub struct TransformExtrapolation;

/// Enables translation extrapolation for an entity, making changes to translation
/// in [`FixedUpdate`] appear smooth.
///
/// Extrapolation only works for entities with velocity components.
/// [`TransformExtrapolationPlugin`] must be added to the app with the appropriate velocity sources.
///
/// See the [`TransformExtrapolationPlugin`] for more information.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
#[require(TranslationEasingState)]
pub struct TranslationExtrapolation;

/// Enables rotation extrapolation for an entity, making changes to rotation
/// in [`FixedUpdate`] appear smooth.
///
/// Extrapolation only works for entities with velocity components.
/// [`TransformExtrapolationPlugin`] must be added to the app with the appropriate velocity sources.
///
/// See the [`TransformExtrapolationPlugin`] for more information.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
#[require(RotationEasingState)]
pub struct RotationExtrapolation;

/// Resets the translation to the start of the extrapolation at the beginning of the fixed timestep
/// to match the true position from the end of the previous fixed tick.
fn reset_translation_extrapolation(
    mut query: Query<
        (&mut Transform, &TranslationEasingState),
        (With<TranslationExtrapolation>, Without<NoTranslationEasing>),
    >,
) {
    for (mut transform, translation_easing) in &mut query {
        if let Some(start) = translation_easing.start {
            transform.translation = start;
        }
    }
}

/// Resets the rotation to the start of the extrapolation at the beginning of the fixed timestep
/// to match the true position from the end of the previous fixed tick.
fn reset_rotation_extrapolation(
    mut query: Query<
        (&mut Transform, &RotationEasingState),
        (With<RotationExtrapolation>, Without<NoRotationEasing>),
    >,
) {
    for (mut transform, rotation_easing) in &mut query {
        if let Some(start) = rotation_easing.start {
            transform.rotation = start;
        }
    }
}

/// Updates the start and end states of the extrapolation for the next fixed timestep.
fn update_translation_extrapolation_states<V: VelocitySource>(
    mut query: Query<
        (&Transform, &mut TranslationEasingState, &V::Current),
        (With<TranslationExtrapolation>, Without<NoTranslationEasing>),
    >,
    time: Res<Time>,
) {
    let delta_secs = time.delta_secs();

    for (transform, mut translation_easing, end_vel) in &mut query {
        translation_easing.start = Some(transform.translation);

        // Extrapolate the next state based on the current state and velocities.
        let lin_vel = <V::Item<'static, 'static> as VelocitySourceItem<V>>::current(end_vel);
        translation_easing.end = Some(transform.translation + lin_vel * delta_secs);
    }
}

/// Updates the start and end states of the extrapolation for the next fixed timestep.
fn update_rotation_extrapolation_states<V: VelocitySource>(
    mut query: Query<
        (&Transform, &mut RotationEasingState, &V::Current),
        (With<RotationExtrapolation>, Without<NoRotationEasing>),
    >,
    time: Res<Time>,
) {
    let delta_secs = time.delta_secs();

    for (transform, mut rotation_easing, end_vel) in &mut query {
        rotation_easing.start = Some(transform.rotation);

        // Extrapolate the next state based on the current state and velocities.
        let ang_vel = <V::Item<'static, 'static> as VelocitySourceItem<V>>::current(end_vel);
        let scaled_axis = ang_vel * delta_secs;
        rotation_easing.end = Some(Quat::from_scaled_axis(scaled_axis) * transform.rotation);
    }
}
