use bevy::reflect::{PartialReflect, ReflectFromReflect, TypeRegistration};

/// Attempts to clone a [`PartialReflect`] value using various methods.
///
/// This first attempts to clone via [`PartialReflect::reflect_clone`].
/// then falls back to [`ReflectFromReflect::from_reflect`],
/// and finally [`PartialReflect::to_dynamic`] if the first two methods fail.
///
/// This helps ensure that the original type and type data is retained,
/// and only returning a dynamic type if all other methods fail.
///
/// `None` when all three fail. Before Bevy 0.20 `to_dynamic` panicked on a
/// value holding an opaque type it could not clone, which took the whole save
/// down; now that it reports the failure, the caller skips that one component
/// and the rest of the scene still saves.
pub(crate) fn clone_reflect_value(
    value: &dyn PartialReflect,
    type_registration: &TypeRegistration,
) -> Option<Box<dyn PartialReflect>> {
    if let Ok(cloned) = value.reflect_clone() {
        return Some(cloned.into_partial_reflect());
    }
    if let Some(from) = type_registration
        .data::<ReflectFromReflect>()
        .and_then(|fr| fr.from_reflect(value.as_partial_reflect()))
    {
        return Some(from.into_partial_reflect());
    }
    match value.to_dynamic() {
        Ok(dynamic) => Some(dynamic),
        Err(e) => {
            bevy::log::warn!(
                "scene save skipped a `{}`: it cannot be cloned for saving ({e})",
                type_registration.type_info().type_path()
            );
            None
        }
    }
}
