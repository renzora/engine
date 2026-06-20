//! # `bevy_transform_interpolation`
//!
//! A drop-in [`Transform`] interpolation solution for fixed timesteps for the [Bevy game engine](https://bevyengine.org).
//!
//! ## Features
//!
//! - Automatically smooth out movement in [`FixedPreUpdate`], [`FixedUpdate`], and [`FixedPostUpdate`].
//! - Support for both [`Transform`] [interpolation](TransformInterpolationPlugin) and [extrapolation](TransformExtrapolationPlugin).
//! - Granularly ease individual [`Transform`] properties to reduce unnecessary computation.
//! - Apply easing to specific entities or to all entities.
//! - Works out of the box with physics engines using fixed timesteps.
//! - Optional [Hermite interpolation][`TransformHermiteEasingPlugin`] to produce more natural and accurate movement that considers velocity.
//! - Extensible with custom easing backends.
//!
//! ## Getting Started
//!
//! First, add `bevy_transform_interpolation` as a dependency in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! bevy_transform_interpolation = "0.4"
//! ```
//!
//! To enable [`Transform`] interpolation, add the [`TransformInterpolationPlugin`] to your app:
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_transform_interpolation::prelude::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins((DefaultPlugins, TransformInterpolationPlugin::default()))
//!         // ...other plugins, resources, and systems
//!         .run();
//! }
//! ```
//!
//! By default, interpolation is only performed for entities with the [`TransformInterpolation`] component:
//!
//! ```
//! # use bevy::prelude::*;
//! # use bevy_transform_interpolation::prelude::*;
//! #
//! fn setup(mut commands: Commands) {
//!     // Interpolate the entire transform: translation, rotation, and scale.
//!     commands.spawn((
//!         Transform::default(),
//!         TransformInterpolation,
//!     ));
//! }
//! ```
//!
//! Now, any changes made to the [`Transform`] of the entity in [`FixedPreUpdate`], [`FixedUpdate`], or [`FixedPostUpdate`]
//! will automatically be interpolated in between fixed timesteps.
//!
//! If you want *all* entities with a [`Transform`] to be interpolated by default, you can use
//! [`TransformInterpolationPlugin::interpolate_all()`]:
//!
//! ```no_run
//! # use bevy::prelude::*;
//! # use bevy_transform_interpolation::prelude::*;
//! #
//! fn main() {
//!    App::new()
//!       .add_plugins(TransformInterpolationPlugin::interpolate_all())
//! #     .add_plugins(bevy::time::TimePlugin::default())
//!       // ...
//!       .run();
//! }
//! ```
//!
//! See the documentation of the [`TransformInterpolationPlugin`] for a more detailed overview of what it can do.
//!
//! ## Advanced Usage
//!
//! For a lot of applications, the functionality shown in the [Getting Started](#getting-started) guide might be all you need!
//! However, `bevy_transform_interpolation` has a lot more to offer:
//!
//! - Granularly ease individual properties of the transform with [`TranslationInterpolation`], [`RotationInterpolation`], and [`ScaleInterpolation`].
//! - Opt out of transform easing for individual entities with [`NoTranslationEasing`], [`NoRotationEasing`], and [`NoScaleEasing`].
//! - Use extrapolation instead of interpolation with the [`TransformExtrapolationPlugin`] and its related components.
//! - Use Hermite interpolation for more natural and accurate movement with the [`TransformHermiteEasingPlugin`].
//! - Implement custom easing backends for your specific needs.
//!
//! ## How Does It Work?
//!
//! Internally, `bevy_transform_interpolation` simply maintains components that store the `start` and `end` of the interpolation.
//! For example, translation uses the following component for easing the movement:
//!
//! ```
//! # use bevy::prelude::*;
//! #
//! pub struct TranslationEasingState {
//!     pub start: Option<Vec3>,
//!     pub end: Option<Vec3>,
//! }
//! ```
//!
//! The states are updated by the [`TransformInterpolationPlugin`] or [`TransformExtrapolationPlugin`]
//! depending on whether the entity has [`TransformInterpolation`] or [`TransformExtrapolation`] components.
//!
//! If interpolation is used:
//!
//! - In [`FixedFirst`], `start` is set to the current [`Transform`].
//! - In [`FixedLast`], `end` is set to the current [`Transform`].
//!
//! If extrapolation is used:
//!
//! - In [`FixedLast`], `start` is set to the current [`Transform`], and `end` is set to the [`Transform`] predicted based on velocity.
//!
//! At the start of the [`FixedFirst`] schedule, the states are reset to `None`. If the [`Transform`] is detected to have changed
//! since the last easing run but *outside* of the fixed timestep schedules, the easing is also reset to `None` to prevent overwriting the change.
//!
//! The actual easing is performed in [`RunFixedMainLoop`], right after [`FixedMain`](bevy::app::FixedMain), before [`Update`].
//! By default, linear interpolation (`lerp`) is used for translation and scale, and spherical linear interpolation (`slerp`)
//! is used for rotation.
//!
//! However, thanks to the modular and flexible architecture, other easing methods can also be used.
//! The [`TransformHermiteEasingPlugin`] provides an easing backend using Hermite interpolation,
//! overwriting the linear interpolation for specific entities with the [`NonlinearTranslationEasing`]
//! and [`NonlinearRotationEasing`] marker components. Custom easing solutions can be implemented using the same pattern.
//!
//! [`TransformHermiteEasingPlugin`]: crate::hermite::TransformHermiteEasingPlugin

#![no_std]
#![allow(clippy::needless_doctest_main)]
#![expect(clippy::type_complexity)]
#![warn(missing_docs)]

// Core interpolation and extrapolation plugins
pub mod extrapolation;
pub mod interpolation;

// Easing backends
// TODO: Catmull-Rom (like Hermite interpolation, but velocity is estimated from four points)
pub mod hermite;

/// The prelude.
///
/// This includes the most common types in this crate, re-exported for your convenience.
pub mod prelude {
    #[doc(inline)]
    pub use crate::{
        NoRotationEasing, NoScaleEasing, NoTransformEasing, NoTranslationEasing,
        TransformEasingPlugin,
        extrapolation::*,
        hermite::{
            RotationHermiteEasing, TransformHermiteEasing, TransformHermiteEasingPlugin,
            TranslationHermiteEasing,
        },
        interpolation::*,
    };
}

use core::marker::PhantomData;

// For doc links.
#[allow(unused_imports)]
use extrapolation::*;
#[allow(unused_imports)]
use interpolation::*;

use bevy::{
    ecs::{change_detection::Tick, query::QueryData, system::SystemChangeTick},
    prelude::*,
};

/// A plugin for applying easing to [`Transform`] changes, making movement in [`FixedUpdate`] appear smooth.
///
/// On its own, this plugin does *not* perform any automatic interpolation. It only performs easing
/// between the `start` and `end` states of the [`TranslationEasingState`], [`RotationEasingState`], and [`ScaleEasingState`]
/// components, and is responsible for resetting them at appropriate times.
///
/// To actually perform automatic easing, an easing backend that updates the `start` and `end` states must be used.
/// The [`TransformInterpolationPlugin`] is provided for transform interpolation, but custom backends can also be implemented.
#[derive(Debug, Default)]
pub struct TransformEasingPlugin;

impl Plugin for TransformEasingPlugin {
    fn build(&self, app: &mut App) {
        // Register easing components.
        app.register_type::<(
            TranslationEasingState,
            RotationEasingState,
            ScaleEasingState,
            NoTranslationEasing,
            NoRotationEasing,
            NoScaleEasing,
        )>();

        app.init_resource::<LastEasingTick>();

        // Reset easing states and update start values at the start of the fixed timestep.
        app.configure_sets(
            FixedFirst,
            (
                TransformEasingSystems::Reset,
                TransformEasingSystems::UpdateStart,
            )
                .chain(),
        );

        // Update end values at the end of the fixed timestep.
        app.configure_sets(FixedLast, TransformEasingSystems::UpdateEnd);

        // Perform transform easing right after the fixed timestep, before `Update`.
        app.configure_sets(
            RunFixedMainLoop,
            (
                TransformEasingSystems::Ease,
                TransformEasingSystems::UpdateEasingTick,
            )
                .chain()
                .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
        );

        // Reset easing states.
        app.add_systems(
            FixedFirst,
            (
                reset_translation_easing,
                reset_rotation_easing,
                reset_scale_easing,
            )
                .chain()
                .in_set(TransformEasingSystems::Reset),
        );

        app.add_systems(
            RunFixedMainLoop,
            reset_easing_states_on_transform_change.before(TransformEasingSystems::Ease),
        );

        // Perform easing.
        app.add_systems(
            RunFixedMainLoop,
            (ease_translation_lerp, ease_rotation_slerp, ease_scale_lerp)
                .in_set(TransformEasingSystems::Ease),
        );

        // Update the last easing tick.
        app.add_systems(
            RunFixedMainLoop,
            update_last_easing_tick.in_set(TransformEasingSystems::UpdateEasingTick),
        );
    }
}

/// System sets for easing transform.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransformEasingSystems {
    /// Resets easing states to `None` at the start of the fixed timestep.
    Reset,
    /// Updates the `start` values for easing at the start of the fixed timestep.
    UpdateStart,
    /// Updates the `end` values for easing at the end of the fixed timestep.
    UpdateEnd,
    /// Eases the transform values in between the `start` and `end` states.
    /// Runs in [`RunFixedMainLoop`], right after [`FixedMain`](bevy::app::FixedMain), before [`Update`].
    Ease,
    /// Updates [`LastEasingTick`], the last tick when easing was performed.
    UpdateEasingTick,
}

/// A deprecated alias for [`TransformEasingSystems`].
#[deprecated(since = "0.3.0", note = "Renamed to `TransformEasingSystems`")]
pub type TransformEasingSet = TransformEasingSystems;

/// A resource that stores the last tick when easing was performed.
#[derive(Resource, Clone, Copy, Debug, Default, Deref, DerefMut)]
pub struct LastEasingTick(Tick);

/// Explicitly marks this entity as having no transform easing, disabling interpolation and/or extrapolation.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
#[require(NoTranslationEasing, NoRotationEasing, NoScaleEasing)]
pub struct NoTransformEasing;

/// Explicitly marks this entity as having no translation easing, disabling interpolation and/or extrapolation.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct NoTranslationEasing;

/// Explicitly marks this entity as having no rotation easing, disabling interpolation and/or extrapolation.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct NoRotationEasing;

/// Explicitly marks this entity as having no scale easing, disabling interpolation and/or extrapolation.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct NoScaleEasing;

/// A marker component that indicates that the entity has non-linear translation easing,
/// and linear easing should not be applied.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct NonlinearTranslationEasing;

/// A marker component that indicates that the entity has non-linear rotation easing,
/// and linear easing should not be applied.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct NonlinearRotationEasing;

/// A [`QueryData`] type for specifying the components that store velocity for easing.
/// Required for [`TransformExtrapolationPlugin`] and [`TransformHermiteEasingPlugin`].
///
/// [`TransformExtrapolationPlugin`]: crate::extrapolation::TransformExtrapolationPlugin
/// [`TransformHermiteEasingPlugin`]: crate::hermite::TransformHermiteEasingPlugin
///
/// # Example
///
/// ```
/// use bevy::{ecs::query::QueryData, prelude::*};
/// use bevy_transform_interpolation::VelocitySource;
///
/// // Velocity components
///
/// #[derive(Component)]
/// struct LinearVelocity(Vec3);
///
/// #[derive(Component)]
/// struct PreviousLinearVelocity(Vec3);
///
/// #[derive(Component)]
/// struct AngularVelocity(Vec3);
///
/// #[derive(Component)]
/// struct PreviousAngularVelocity(Vec3);
///
/// // Velocity source for easing that uses linear velocity
/// #[derive(QueryData)]
/// struct LinVelSource;
///
/// impl VelocitySource for LinVelSource {
///     type Previous = PreviousLinearVelocity;
///     type Current = LinearVelocity;
///
///     fn previous(previous: &Self::Previous) -> Vec3 {
///         previous.0
///     }
///
///     fn current(current: &Self::Current) -> Vec3 {
///         current.0
///     }
/// }
///
/// // Velocity source for easing that uses angular velocity
/// #[derive(QueryData)]
/// struct AngVelSource;
///
/// impl VelocitySource for AngVelSource {
///     type Previous = PreviousAngularVelocity;
///     type Current = AngularVelocity;
///
///     fn previous(previous: &Self::Previous) -> Vec3 {
///         previous.0
///     }
///
///     fn current(current: &Self::Current) -> Vec3 {
///         current.0
///     }
/// }
/// ```
///
/// Some forms of easing such as extrapolation may not require the previous velocity.
/// In such cases, the `Previous` component can be set to `()`, and `previous` can simply return `Vec3::ZERO`.
pub trait VelocitySource: QueryData + Send + Sync + 'static {
    /// The component that stores the previous velocity.
    ///
    /// This is not required for all easing backends, such as extrapolation.
    /// In such cases, this can be set to `()`.
    type Previous: Component;

    /// The component that stores the current velocity.
    type Current: Component;

    /// Returns the previous velocity.
    ///
    /// This is not required for all easing backends, such as extrapolation.
    /// In such cases, this can return `Vec3::ZERO`.
    fn previous(start: &Self::Previous) -> Vec3;

    /// Returns the current velocity.
    fn current(end: &Self::Current) -> Vec3;
}

trait VelocitySourceItem<V>
where
    V: VelocitySource,
{
    fn previous(start: &V::Previous) -> Vec3;
    fn current(end: &V::Current) -> Vec3;
}

impl<V: VelocitySource> VelocitySourceItem<V> for V::Item<'_, '_> {
    fn previous(start: &V::Previous) -> Vec3 {
        V::previous(start)
    }

    fn current(end: &V::Current) -> Vec3 {
        V::current(end)
    }
}

// Required so that `()` can be used as a "null" velocity source despite it not being a component itself.
// This can be useful if you only want to use Hermite interpolation for rotation, for example.
//
// This must be public, because `VelocitySource::Start` and `VelocitySource::End` are public interfaces,
// but you can't actually create this component since the stored value is private and there are no constructors.
#[derive(Component)]
#[doc(hidden)]
pub struct DummyComponent(PhantomData<()>);

impl VelocitySource for () {
    type Previous = DummyComponent;
    type Current = DummyComponent;

    fn previous(_: &Self::Previous) -> Vec3 {
        Vec3::ZERO
    }

    fn current(_: &Self::Current) -> Vec3 {
        Vec3::ZERO
    }
}

/// Stores the start and end states used for interpolating the translation of an entity.
/// The change in translation is smoothed from `start` to `end` in between [`FixedUpdate`] runs.
///
/// On its own, this component is not updated automatically. Enable an easing backend
/// such as the [`TransformInterpolationPlugin`] to perform automatic interpolation.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
#[reflect(Component, Debug, Default)]
pub struct TranslationEasingState {
    /// The start translation for the interpolation.
    pub start: Option<Vec3>,
    /// The end translation for the interpolation.
    pub end: Option<Vec3>,
}

/// Stores the start and end states used for interpolating the rotation of an entity.
/// The change in rotation is smoothed from `start` to `end` in between [`FixedUpdate`] runs.
///
/// On its own, this component is not updated automatically. Enable an easing backend
/// such as the [`TransformInterpolationPlugin`] to perform automatic interpolation.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
#[reflect(Component, Debug, Default)]
pub struct RotationEasingState {
    /// The start rotation for the interpolation.
    pub start: Option<Quat>,
    /// The end rotation for the interpolation.
    pub end: Option<Quat>,
}

/// Stores the start and end states used for interpolating the scale of an entity.
/// The change in scale is smoothed from `start` to `end` in between [`FixedUpdate`] runs.
///
/// On its own, this component is not updated automatically. Enable an easing backend
/// such as the [`TransformInterpolationPlugin`] to perform automatic interpolation.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
#[reflect(Component, Debug, Default)]
pub struct ScaleEasingState {
    /// The start scale for the interpolation.
    pub start: Option<Vec3>,
    /// The end scale for the interpolation.
    pub end: Option<Vec3>,
}

fn update_last_easing_tick(
    mut last_easing_tick: ResMut<LastEasingTick>,
    system_change_tick: SystemChangeTick,
) {
    *last_easing_tick = LastEasingTick(system_change_tick.this_run());
}

/// Resets the easing states to `None` when [`Transform`] is modified outside of the fixed timestep schedules
/// or interpolation logic. This makes it possible to "teleport" entities in schedules like [`Update`].
#[allow(clippy::type_complexity, private_interfaces)]
pub fn reset_easing_states_on_transform_change(
    mut query: Query<
        (
            Ref<Transform>,
            Option<&mut TranslationEasingState>,
            Option<&mut RotationEasingState>,
            Option<&mut ScaleEasingState>,
        ),
        (
            Changed<Transform>,
            Or<(
                With<TranslationEasingState>,
                With<RotationEasingState>,
                With<ScaleEasingState>,
            )>,
        ),
    >,
    last_easing_tick: Res<LastEasingTick>,
    system_change_tick: SystemChangeTick,
) {
    let this_run = system_change_tick.this_run();

    query.par_iter_mut().for_each(
        |(transform, translation_easing, rotation_easing, scale_easing)| {
            let last_changed = transform.last_changed();
            let is_user_change = last_changed.is_newer_than(last_easing_tick.0, this_run);

            if !is_user_change {
                return;
            }

            if let Some(mut translation_easing) = translation_easing
                && let (Some(start), Some(end)) = (translation_easing.start, translation_easing.end)
                && transform.translation != start
                && transform.translation != end
            {
                translation_easing.start = None;
                translation_easing.end = None;
            }
            if let Some(mut rotation_easing) = rotation_easing
                && let (Some(start), Some(end)) = (rotation_easing.start, rotation_easing.end)
                && transform.rotation != start
                && transform.rotation != end
            {
                rotation_easing.start = None;
                rotation_easing.end = None;
            }
            if let Some(mut scale_easing) = scale_easing
                && let (Some(start), Some(end)) = (scale_easing.start, scale_easing.end)
                && transform.scale != start
                && transform.scale != end
            {
                scale_easing.start = None;
                scale_easing.end = None;
            }
        },
    );
}

/// Resets the `start` and `end` states for translation interpolation.
fn reset_translation_easing(mut query: Query<&mut TranslationEasingState>) {
    for mut easing in &mut query {
        easing.start = None;
        easing.end = None;
    }
}

/// Resets the `start` and `end` states for rotation interpolation.
fn reset_rotation_easing(mut query: Query<&mut RotationEasingState>) {
    for mut easing in &mut query {
        easing.start = None;
        easing.end = None;
    }
}

/// Resets the `start` and `end` states for scale interpolation.
fn reset_scale_easing(mut query: Query<&mut ScaleEasingState>) {
    for mut easing in &mut query {
        easing.start = None;
        easing.end = None;
    }
}

/// Eases the translations of entities with linear interpolation.
fn ease_translation_lerp(
    mut query: Query<
        (&mut Transform, &TranslationEasingState),
        (
            Without<NonlinearTranslationEasing>,
            Without<NoTranslationEasing>,
        ),
    >,
    time: Res<Time<Fixed>>,
) {
    let overstep = time.overstep_fraction();

    query.iter_mut().for_each(|(mut transform, interpolation)| {
        if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
            transform.translation = start.lerp(end, overstep);
        }
    });
}

/// Eases the rotations of entities with spherical linear interpolation.
fn ease_rotation_slerp(
    mut query: Query<
        (&mut Transform, &RotationEasingState),
        (Without<NonlinearRotationEasing>, Without<NoRotationEasing>),
    >,
    time: Res<Time<Fixed>>,
) {
    let overstep = time.overstep_fraction();

    query
        .par_iter_mut()
        .for_each(|(mut transform, interpolation)| {
            if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
                // Note: `slerp` will always take the shortest path, but when the two rotations are more than
                // 180 degrees apart, this can cause visual artifacts as the rotation "flips" to the other side.
                transform.rotation = start.slerp(end, overstep);
            }
        });
}

/// Eases the scales of entities with linear interpolation.
fn ease_scale_lerp(
    mut query: Query<(&mut Transform, &ScaleEasingState), Without<NoScaleEasing>>,
    time: Res<Time<Fixed>>,
) {
    let overstep = time.overstep_fraction();

    query.iter_mut().for_each(|(mut transform, interpolation)| {
        if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
            transform.scale = start.lerp(end, overstep);
        }
    });
}
