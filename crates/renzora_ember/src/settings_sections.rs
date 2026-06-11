//! Plugin-extensible settings sections.
//!
//! Plugins call [`RegisterSettingsSection::register_settings_section`] to add
//! a section to the editor Settings overlay's **Plugins** tab. The settings
//! UI wraps each section in its standard chrome (icon + title header) and
//! calls `build` to fill the body — the same build-once + reactive-bindings
//! model as panel content.
//!
//! This mirrors the panel split: metadata + content builder live here in
//! ember (both the settings overlay and plugin dylibs link it); the overlay
//! crate just iterates the registry.

use bevy::prelude::*;

use crate::font::EmberFonts;

pub type SettingsSectionBuild = Box<dyn Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync>;

pub struct SettingsSection {
    pub id: String,
    pub title: String,
    /// Phosphor icon name for the section header.
    pub icon: String,
    /// Builds the section body (runs once per overlay open; use reactive
    /// bindings for live values).
    pub build: SettingsSectionBuild,
}

/// Sections shown on the Settings overlay's Plugins tab, in registration
/// order.
#[derive(Resource, Default)]
pub struct SettingsSectionRegistry(pub Vec<SettingsSection>);

pub trait RegisterSettingsSection {
    /// Register (or replace, by `id`) a Plugins-tab settings section.
    fn register_settings_section<F>(
        &mut self,
        id: &str,
        title: &str,
        icon: &str,
        build: F,
    ) -> &mut Self
    where
        F: Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static;
}

impl RegisterSettingsSection for App {
    fn register_settings_section<F>(
        &mut self,
        id: &str,
        title: &str,
        icon: &str,
        build: F,
    ) -> &mut Self
    where
        F: Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static,
    {
        self.init_resource::<SettingsSectionRegistry>();
        let mut reg = self.world_mut().resource_mut::<SettingsSectionRegistry>();
        reg.0.retain(|s| s.id != id);
        reg.0.push(SettingsSection {
            id: id.to_string(),
            title: title.to_string(),
            icon: icon.to_string(),
            build: Box::new(build),
        });
        self
    }
}
