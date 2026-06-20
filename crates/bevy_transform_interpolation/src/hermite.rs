//! Hermite interpolation for [`Transform`] easing.

use core::{f32::consts::TAU, marker::PhantomData};

use bevy::prelude::*;
use ops::FloatPow;

use crate::{
    NoRotationEasing, NoTranslationEasing, NonlinearRotationEasing, NonlinearTranslationEasing,
    RotationEasingState, TransformEasingSystems, TranslationEasingState, VelocitySource,
    VelocitySourceItem,
};

/// A Hermite interpolation plugin for [`Transform`] easing.
///
/// By default, [`TransformInterpolationPlugin`] and [`TransformExtrapolationPlugin`]
/// use *linear interpolation* (`lerp`) for easing translation and scale,
/// and *spherical linear interpolation* (`slerp`) for easing rotation.
/// This is computationally efficient and works well for most cases.
///
/// However, for more accurate and reliable easing that works at arbitrary velocities,
/// it may be preferable to use *Hermite interpolation*. It uses both position and velocity information
/// to estimate the trajectories of entities, producing smoother results.
///
/// This plugin should be used alongside the [`TransformInterpolationPlugin`] and/or [`TransformExtrapolationPlugin`].
/// The [`TransformEasingPlugin`] is also required, and it is automatically added if not already present in the app.
///
/// [`TransformInterpolationPlugin`]: crate::interpolation::TransformInterpolationPlugin
/// [`TransformExtrapolationPlugin`]: crate::extrapolation::TransformExtrapolationPlugin
/// [`TransformEasingPlugin`]: crate::TransformEasingPlugin
///
/// # Usage
///
/// Hermite interpolation requires velocity to produce accurate curves.
/// Instead of providing its own velocity components, the [`TransformHermiteEasingPlugin`]
/// lets you specify your own velocity components that you manage yourself.
///
/// First, make sure you have components for the previous and current velocity, and implement
/// the [`VelocitySource`] trait on a [`QueryData`] type:
///
/// ```
/// use bevy::{ecs::query::QueryData, prelude::*};
/// use bevy_transform_interpolation::VelocitySource;
///
/// #[derive(Component, Default)]
/// struct PreviousLinearVelocity(Vec3);
///
/// #[derive(Component, Default)]
/// struct PreviousAngularVelocity(Vec3);
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
///     type Previous = PreviousLinearVelocity;
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
///     type Previous = PreviousAngularVelocity;
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
/// Then, add the [`TransformHermiteEasingPlugin`] to the app with the velocity sources,
/// along with the [`TransformInterpolationPlugin`] and/or [`TransformExtrapolationPlugin`]:
///
/// ```no_run
/// use bevy::{ecs::query::QueryData, prelude::*};
/// use bevy_transform_interpolation::{prelude::*, VelocitySource};
/// #
/// # #[derive(Component, Default)]
/// # struct PreviousLinearVelocity(Vec3);
/// #
/// # #[derive(Component, Default)]
/// # struct PreviousAngularVelocity(Vec3);
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
/// #     // Components storing the previous and current velocities.
/// #     type Previous = PreviousLinearVelocity;
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
/// #     type Previous = PreviousAngularVelocity;
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
///        TransformHermiteEasingPlugin::<LinVelSource, AngVelSource>::default(),
/// #      bevy::time::TimePlugin::default(),
///    ));
///
///    // Optional: Insert velocity components automatically for entities with Hermite interpolation.
///    app.register_required_components::<TranslationHermiteEasing, LinearVelocity>();
///    app.register_required_components::<TranslationHermiteEasing, PreviousLinearVelocity>();
///    app.register_required_components::<RotationHermiteEasing, AngularVelocity>();
///    app.register_required_components::<RotationHermiteEasing, PreviousAngularVelocity>();
///
///    // ...
///
///    app.run();
/// }
/// ```
///
/// Hermite interpolation can now be used for any interpolated or extrapolated entity
/// that has the velocity components by adding the [`TransformHermiteEasing`] component:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_transform_interpolation::prelude::*;
/// #
/// fn setup(mut commands: Commands) {
///     // Use Hermite interpolation for interpolating translation and rotation.
///     commands.spawn((
///         Transform::default(),
///         TransformInterpolation,
///         TransformHermiteEasing,
///     ));
/// }
/// ```
///
/// Hermite interpolation can also be used for translation and rotation separately:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_transform_interpolation::prelude::*;
/// #
/// fn setup(mut commands: Commands) {
///     // Use Hermite interpolation for interpolating translation.
///     commands.spawn((
///         Transform::default(),
///         TranslationInterpolation,
///         TranslationHermiteEasing,
///     ));
///
///     // Use Hermite interpolation for interpolating rotation.
///     commands.spawn((
///         Transform::default(),
///         RotationInterpolation,
///         RotationHermiteEasing,
///     ));
/// }
/// ```
///
/// [`QueryData`]: bevy::ecs::query::QueryData
#[derive(Debug)]
pub struct TransformHermiteEasingPlugin<LinVel: VelocitySource, AngVel: VelocitySource>(
    PhantomData<LinVel>,
    PhantomData<AngVel>,
);

impl<LinVel: VelocitySource, AngVel: VelocitySource> Default
    for TransformHermiteEasingPlugin<LinVel, AngVel>
{
    fn default() -> Self {
        Self(PhantomData, PhantomData)
    }
}

impl<LinVel: VelocitySource, AngVel: VelocitySource> Plugin
    for TransformHermiteEasingPlugin<LinVel, AngVel>
{
    fn build(&self, app: &mut App) {
        // Register components.
        app.register_type::<(
            TransformHermiteEasing,
            TranslationHermiteEasing,
            RotationHermiteEasing,
        )>();

        // Mark entities with Hermite interpolation as having nonlinear easing to disable linear easing.
        let _ = app
            .try_register_required_components::<TranslationHermiteEasing, NonlinearTranslationEasing>();
        let _ = app
            .try_register_required_components::<RotationHermiteEasing, NonlinearRotationEasing>();

        // Perform easing.
        app.add_systems(
            RunFixedMainLoop,
            (
                ease_translation_hermite::<LinVel>,
                ease_rotation_hermite::<AngVel>,
            )
                .in_set(TransformEasingSystems::Ease),
        );
    }
}

/// Enables [Hermite interpolation](TransformHermiteEasingPlugin) for the easing of the [`Transform`] of an entity.
/// Must be used together with either [`TransformInterpolation`] or [`TransformExtrapolation`].
///
/// For the interpolation to work, the entity must have velocity components that are updated every frame,
/// and the app must have a [`TransformHermiteEasingPlugin`] with the appropriate velocity sources added.
///
/// See the [`TransformHermiteEasingPlugin`] for more information.
///
/// [`TransformInterpolation`]: crate::interpolation::TransformInterpolation
/// [`TransformExtrapolation`]: crate::extrapolation::TransformExtrapolation
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
#[require(TranslationHermiteEasing, RotationHermiteEasing)]
pub struct TransformHermiteEasing;

/// Enables [Hermite interpolation](TransformHermiteEasingPlugin) for the easing of the translation of an entity.
/// Must be used together with [`TranslationInterpolation`] or [`TranslationExtrapolation`].
///
/// For the interpolation to work, the entity must have a linear velocity component that is updated every frame,
/// and the app must have a [`TransformHermiteEasingPlugin`] with the appropriate velocity source added.
///
/// See the [`TransformHermiteEasingPlugin`] for more information.
///
/// [`TranslationInterpolation`]: crate::interpolation::TranslationInterpolation
/// [`TranslationExtrapolation`]: crate::extrapolation::TranslationExtrapolation
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct TranslationHermiteEasing;

/// Enables [Hermite interpolation](TransformHermiteEasingPlugin) for the easing of the rotation of an entity.
/// Must be used together with [`RotationInterpolation`] or [`RotationExtrapolation`].
///
/// For the interpolation to work, the entity must have an angular velocity component that is updated every frame,
/// and the app must have a [`TransformHermiteEasingPlugin`] with the appropriate velocity source added.
///
/// See the [`TransformHermiteEasingPlugin`] for more information.
///
/// [`RotationInterpolation`]: crate::interpolation::RotationInterpolation
/// [`RotationExtrapolation`]: crate::extrapolation::RotationExtrapolation
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct RotationHermiteEasing;

/// Eases the translations of entities with Hermite interpolation.
fn ease_translation_hermite<V: VelocitySource>(
    mut query: Query<
        (
            &mut Transform,
            &TranslationEasingState,
            &V::Previous,
            &V::Current,
        ),
        Without<NoTranslationEasing>,
    >,
    time: Res<Time<Fixed>>,
) {
    let overstep = time.overstep_fraction();
    let delta_secs = time.delta_secs();

    query
        .par_iter_mut()
        .for_each(|(mut transform, interpolation, start_vel, end_vel)| {
            if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
                let vel0 =
                    <V::Item<'static, 'static> as VelocitySourceItem<V>>::previous(start_vel);
                let vel1 = <V::Item<'static, 'static> as VelocitySourceItem<V>>::current(end_vel);
                transform.translation =
                    hermite_vec3(start, end, delta_secs * vel0, delta_secs * vel1, overstep);
            }
        });
}

/// Eases the rotations of entities with Hermite interpolation.
fn ease_rotation_hermite<V: VelocitySource>(
    mut query: Query<
        (
            &mut Transform,
            &RotationEasingState,
            &V::Previous,
            &V::Current,
        ),
        Without<NoRotationEasing>,
    >,
    time: Res<Time<Fixed>>,
) {
    let overstep = time.overstep_fraction();
    let delta_secs = time.delta_secs();

    query
        .par_iter_mut()
        .for_each(|(mut transform, interpolation, start_vel, end_vel)| {
            if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
                let vel0 =
                    <V::Item<'static, 'static> as VelocitySourceItem<V>>::previous(start_vel);
                let vel1 = <V::Item<'static, 'static> as VelocitySourceItem<V>>::current(end_vel);
                transform.rotation = hermite_quat(
                    start,
                    end,
                    delta_secs * vel0,
                    delta_secs * vel1,
                    overstep,
                    true,
                );
            }
        });
}

/// Performs a cubic Hermite interpolation between two vectors `p0` and `p1` with velocities `v0` and `v1`
/// based on the value at `t`.
///
/// When `t` is `0.0`, the result will be equal to `p0`. When `t` is `1.0`, the result will be equal to `p1`.
pub fn hermite_vec3(p0: Vec3, p1: Vec3, v0: Vec3, v1: Vec3, t: f32) -> Vec3 {
    // Reference:
    //
    // Holden, D. "Cubic Interpolation of Quaternions"
    // https://theorangeduck.com/page/cubic-interpolation-quaternions
    //
    // The article is mostly about quaternions, but also describes Hermite interpolation for vectors.
    // For quaternions, we use a different approach. See `hermite_quat`.

    let t2 = t * t;
    let t3 = t2 * t;

    // Polynomial coefficients
    let b0 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let b1 = 3.0 * t2 - 2.0 * t3;
    let b2 = t3 - 2.0 * t2 + t;
    let b3 = t3 - t2;

    b0 * p0 + b1 * p1 + b2 * v0 + b3 * v1
}

/// Performs a cubic Hermite interpolation between quaternions `q0` and `q1`
/// with angular velocities `w0` and `w1` based on the value at `t`.
///
/// Both quaternions and angular velocities should be in the global frame.
/// The angular velocities should be normalized such that they represent the angle of rotation
/// over the time step.
///
/// When `t` is `0.0`, the result will be equal to `q0`. When `t` is `1.0`, the result will be equal to `q1`.
///
/// If `unwrap` is `true`, the interpolation will work for arbitrarily large velocities
/// and handle multiple full revolutions correctly. This is a bit more expensive,
/// but can be important for high angular velocities.
pub fn hermite_quat(qa: Quat, qb: Quat, w0: Vec3, w1: Vec3, t: f32, unwrap: bool) -> Quat {
    // Reference:
    //
    // Kim M.-J. et al. "A General Construction Scheme for Unit Quaternion Curves with Simple High Order Derivatives".
    // http://graphics.cs.cmu.edu/nsp/course/15-464/Fall05/papers/kimKimShin.pdf
    //
    // Note that the paper's angular velocities are defined in the local frame, but our values
    // are in the global frame, so the order of multiplication for quaternions is reversed.

    let t2 = t * t;
    let t3 = t * t2;

    // Cumulative Bernstein basis polynomials
    let b1 = 1.0 - (1.0 - t).cubed();
    let b2 = 3.0 * t2 - 2.0 * t3;
    let b3 = t3;

    let w0_div_3 = w0 / 3.0;
    let w1_div_3 = w1 / 3.0;

    // Advance by a third from initial rotation, with initial velocity.
    let q1 = Quat::from_scaled_axis(w0_div_3) * qa;

    // Back off by a third from final rotation, with final velocity.
    let q2 = Quat::from_scaled_axis(-w1_div_3) * qb;

    // Calculate fractional rotation needed to go from q0 to q1.
    // q1 = q0 * Quat(w01 / 3)
    let mut w01_div_3 = (q2 * q1.inverse()).to_scaled_axis();

    // Add multiples of 2π to the magnitude of w01 / 3 to minimize
    // its distance to the average of w0 / 3 and w1 / 3.
    if unwrap {
        let average_w_div_3 = w0_div_3.midpoint(w1_div_3);
        let w01_direction = w01_div_3.normalize_or_zero();

        // Closest point along unit vector n from starting point a to target point p, where l is the distance:
        //
        // argmin(l) length(a + l n - p)^2
        //
        // 0 = d/dl length(a + l n - p)^2 = dot([a + l n - p], n)
        //
        // l dot(n, n) = l = dot(p - a, n)

        let extra_angle = w01_direction.dot(average_w_div_3 - w01_div_3);
        w01_div_3 += ops::round(extra_angle / TAU) * TAU * w01_direction;
    }

    // Rotate by b1 * dt / 3 at initial velocity, then by b2 * dt / 3 at w01, then by b3 * dt / 3 at final velocity.
    Quat::from_scaled_axis(b3 * w1_div_3)
        * Quat::from_scaled_axis(b2 * w01_div_3)
        * Quat::from_scaled_axis(b1 * w0_div_3)
        * qa
}
