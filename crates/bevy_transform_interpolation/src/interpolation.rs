//! [`Transform`] interpolation, making movement in [`FixedUpdate`] appear smooth
//! by easing between the old and current [`Transform`] in between fixed timesteps.
//!
//! See the [`TransformInterpolationPlugin`] for more information.

#![allow(clippy::type_complexity)]

use crate::{
    RotationEasingState, ScaleEasingState, TransformEasingSystems, TranslationEasingState,
    prelude::*,
};
use bevy::prelude::*;

/// A plugin for [`Transform`] interpolation, making movement in [`FixedUpdate`] appear smooth.
///
/// Transform interpolation applies easing between the old and current [`Transform`]
/// in between fixed timesteps. This results in movement that looks smooth and accurate,
/// at the cost of rendered positions being slightly behind the "true" gameplay positions.
///
/// This plugin requires the [`TransformEasingPlugin`] to function. It is automatically added
/// if not already present in the app.
///
/// # Usage
///
/// Transform interpolation can be enabled for a given entity by adding the [`TransformInterpolation`] component.
///
/// ```
/// use bevy::prelude::*;
/// use bevy_transform_interpolation::prelude::*;
///
/// fn setup(mut commands: Commands) {
///     // Interpolate the entire transform: translation, rotation, and scale.
///     commands.spawn((
///         Transform::default(),
///         TransformInterpolation,
///     ));
/// }
/// ```
///
/// Now, any changes made to the [`Transform`] of the entity in [`FixedPreUpdate`], [`FixedUpdate`],
/// or [`FixedPostUpdate`] will automatically be smoothed in between fixed timesteps.
///
/// Transform properties can also be interpolated individually by adding the [`TranslationInterpolation`],
/// [`RotationInterpolation`], and [`ScaleInterpolation`] components.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_transform_interpolation::prelude::*;
/// #
/// fn setup(mut commands: Commands) {
///     // Only interpolate translation.
///     commands.spawn((Transform::default(), TranslationInterpolation));
///     
///     // Only interpolate rotation.
///     commands.spawn((Transform::default(), RotationInterpolation));
///     
///     // Only interpolate scale.
///     commands.spawn((Transform::default(), ScaleInterpolation));
///     
///     // Interpolate translation and rotation, but not scale.
///     commands.spawn((
///         Transform::default(),
///         TranslationInterpolation,
///         RotationInterpolation,
///     ));
/// }
/// ```
///
/// If you want *all* entities with a [`Transform`] to be interpolated by default, you can use
/// [`TransformInterpolationPlugin::interpolate_all()`], or set the [`interpolate_translation_all`],
/// [`interpolate_rotation_all`], and [`interpolate_scale_all`] fields.
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use bevy_transform_interpolation::prelude::*;
/// #
/// fn main() {
///    App::new()
///       .add_plugins(TransformInterpolationPlugin {
///           // Interpolate translation and rotation by default, but not scale.
///           interpolate_translation_all: true,
///           interpolate_rotation_all: true,
///           interpolate_scale_all: false,
///       })
///       // ...
///       .run();
/// }
/// ```
///
/// When interpolation is enabled for all entities by default, you can still opt out of it for individual entities
/// by adding the [`NoTransformEasing`] component, or the individual [`NoTranslationEasing`], [`NoRotationEasing`],
/// and [`NoScaleEasing`] components.
///
/// Note that changing [`Transform`] manually in any schedule that *doesn't* use a fixed timestep is also supported,
/// but it is equivalent to teleporting, and disables interpolation for the entity for the remainder of that fixed timestep.
///
/// [`interpolate_translation_all`]: TransformInterpolationPlugin::interpolate_translation_all
/// [`interpolate_rotation_all`]: TransformInterpolationPlugin::interpolate_rotation_all
/// [`interpolate_scale_all`]: TransformInterpolationPlugin::interpolate_scale_all
///
/// # Alternatives
///
/// For games where low latency is crucial for gameplay, such as in some first-person shooters
/// or racing games, the small delay introduced by interpolation may be undesirable. In those cases,
/// one option is to use the [`TransformExtrapolationPlugin`] instea.
///
/// Transform extrapolation predicts future positions based on velocity, and applies easing between
/// the current and predicted [`Transform`]. This results in movement that looks smooth and feels responsive,
/// but can stutter when the prediction is incorrect, such as when velocity changes abruptly.
///
/// # Easing Backends
///
/// By default, transform interpolation uses linear interpolation (`lerp`) for easing translation and scale,
/// and spherical linear interpolation (`slerp`) for easing rotation.
///
/// If the previous and current velocities are also available, it is possible to use [Hermite interpolation]
/// with the [`TransformHermiteEasingPlugin`] to get smoother and more accurate easing. To enable Hermite interpolation,
/// add the [`TransformHermiteEasing`] component to the entity in addition to the core interpolation components.
#[derive(Debug, Default)]
pub struct TransformInterpolationPlugin {
    /// If `true`, translation will be interpolated for all entities with the [`Transform`] component by default.
    ///
    /// This can be overridden for individual entities by adding the [`NoTranslationEasing`] or [`NoTransformEasing`] component.
    pub interpolate_translation_all: bool,
    /// If `true`, rotation will be interpolated for all entities with the [`Transform`] component by default.
    ///
    /// This can be overridden for individual entities by adding the [`NoRotationEasing`] or [`NoTransformEasing`] component.
    pub interpolate_rotation_all: bool,
    /// If `true`, scale will be interpolated for all entities with the [`Transform`] component by default.
    ///
    /// This can be overridden for individual entities by adding the [`NoScaleEasing`] or [`NoTransformEasing`] component.
    pub interpolate_scale_all: bool,
}

impl TransformInterpolationPlugin {
    /// Enables interpolation for translation, rotation, and scale for all entities with the [`Transform`] component.
    ///
    /// This can be overridden for individual entities by adding the [`NoTransformEasing`] component,
    /// or the individual [`NoTranslationEasing`], [`NoRotationEasing`], and [`NoScaleEasing`] components.
    pub const fn interpolate_all() -> Self {
        Self {
            interpolate_translation_all: true,
            interpolate_rotation_all: true,
            interpolate_scale_all: true,
        }
    }
}

impl Plugin for TransformInterpolationPlugin {
    fn build(&self, app: &mut App) {
        // Register components.
        app.register_type::<(
            TranslationInterpolation,
            RotationInterpolation,
            ScaleInterpolation,
        )>();

        app.add_systems(
            FixedFirst,
            (
                complete_translation_easing,
                complete_rotation_easing,
                complete_scale_easing,
            )
                .chain()
                .before(TransformEasingSystems::Reset),
        );

        // Update the start state of the interpolation at the start of the fixed timestep.
        app.add_systems(
            FixedFirst,
            (
                update_translation_interpolation_start,
                update_rotation_interpolation_start,
                update_scale_interpolation_start,
            )
                .chain()
                .in_set(TransformEasingSystems::UpdateStart),
        );

        // Update the end state of the interpolation at the end of the fixed timestep.
        app.add_systems(
            FixedLast,
            (
                update_translation_interpolation_end,
                update_rotation_interpolation_end,
                update_scale_interpolation_end,
            )
                .chain()
                .in_set(TransformEasingSystems::UpdateEnd),
        );

        // Insert interpolation components automatically for all entities with a `Transform`
        // if the corresponding global interpolation is enabled.
        if self.interpolate_translation_all {
            let _ = app.try_register_required_components::<Transform, TranslationInterpolation>();
        }
        if self.interpolate_rotation_all {
            let _ = app.try_register_required_components::<Transform, RotationInterpolation>();
        }
        if self.interpolate_scale_all {
            let _ = app.try_register_required_components::<Transform, ScaleInterpolation>();
        }
    }

    fn finish(&self, app: &mut App) {
        // Add the `TransformEasingPlugin` if it hasn't been added yet.
        if !app.is_plugin_added::<TransformEasingPlugin>() {
            app.add_plugins(TransformEasingPlugin);
        }
    }
}

/// Enables full [`Transform`] interpolation for an entity, making changes to translation,
/// rotation, and scale in [`FixedUpdate`] appear smooth.
///
/// See the [`TransformInterpolationPlugin`] for more information.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
#[require(TranslationInterpolation, RotationInterpolation, ScaleInterpolation)]
pub struct TransformInterpolation;

/// Enables translation interpolation for an entity, making changes to translation
/// in [`FixedUpdate`] appear smooth.
///
/// See the [`TransformInterpolationPlugin`] for more information.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
#[require(TranslationEasingState)]
pub struct TranslationInterpolation;

/// Enables rotation interpolation for an entity, making changes to rotation
/// in [`FixedUpdate`] appear smooth.
///
/// See the [`TransformInterpolationPlugin`] for more information.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
#[require(RotationEasingState)]
pub struct RotationInterpolation;

/// Enables scale interpolation for an entity, making changes to scale
/// in [`FixedUpdate`] appear smooth.
///
/// See the [`TransformInterpolationPlugin`] for more information.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
#[require(ScaleEasingState)]
pub struct ScaleInterpolation;

/// Makes sure the previous translation easing is fully applied before the next easing starts.
fn complete_translation_easing(
    mut query: Query<
        (&mut Transform, &TranslationEasingState),
        (With<TranslationInterpolation>, Without<NoTranslationEasing>),
    >,
) {
    for (mut transform, easing) in &mut query {
        // Make sure the previous easing is fully applied.
        if let Some(end) = easing.end {
            transform.translation = end;
        }
    }
}

/// Makes sure the previous rotation easing is fully applied before the next easing starts.
fn complete_rotation_easing(
    mut query: Query<
        (&mut Transform, &RotationEasingState),
        (With<RotationInterpolation>, Without<NoRotationEasing>),
    >,
) {
    for (mut transform, easing) in &mut query {
        // Make sure the previous easing is fully applied.
        if let Some(end) = easing.end {
            transform.rotation = end;
        }
    }
}

/// Makes sure the previous scale easing is fully applied before the next easing starts.
fn complete_scale_easing(
    mut query: Query<
        (&mut Transform, &ScaleEasingState),
        (With<ScaleInterpolation>, Without<NoScaleEasing>),
    >,
) {
    for (mut transform, easing) in &mut query {
        // Make sure the previous easing is fully applied.
        if let Some(end) = easing.end {
            transform.scale = end;
        }
    }
}

fn update_translation_interpolation_start(
    mut query: Query<
        (&Transform, &mut TranslationEasingState),
        (With<TranslationInterpolation>, Without<NoTranslationEasing>),
    >,
) {
    for (transform, mut easing) in &mut query {
        easing.start = Some(transform.translation);
    }
}

fn update_translation_interpolation_end(
    mut query: Query<
        (&Transform, &mut TranslationEasingState),
        (With<TranslationInterpolation>, Without<NoTranslationEasing>),
    >,
) {
    for (transform, mut easing) in &mut query {
        easing.end = Some(transform.translation);
    }
}

fn update_rotation_interpolation_start(
    mut query: Query<
        (&Transform, &mut RotationEasingState),
        (With<RotationInterpolation>, Without<NoRotationEasing>),
    >,
) {
    for (transform, mut easing) in &mut query {
        easing.start = Some(transform.rotation);
    }
}

fn update_rotation_interpolation_end(
    mut query: Query<
        (&Transform, &mut RotationEasingState),
        (With<RotationInterpolation>, Without<NoRotationEasing>),
    >,
) {
    for (transform, mut easing) in &mut query {
        easing.end = Some(transform.rotation);
    }
}

fn update_scale_interpolation_start(
    mut query: Query<
        (&Transform, &mut ScaleEasingState),
        (With<ScaleInterpolation>, Without<NoScaleEasing>),
    >,
) {
    for (transform, mut easing) in &mut query {
        easing.start = Some(transform.scale);
    }
}

fn update_scale_interpolation_end(
    mut query: Query<
        (&Transform, &mut ScaleEasingState),
        (With<ScaleInterpolation>, Without<NoScaleEasing>),
    >,
) {
    for (transform, mut easing) in &mut query {
        easing.end = Some(transform.scale);
    }
}
