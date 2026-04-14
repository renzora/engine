//! Clipboard for copying component values between entities.

use bevy::prelude::Resource;
use renzora_editor_framework::FieldValue;

/// Holds a snapshot of one component's field values.
#[derive(Resource, Default, Clone)]
pub struct ComponentClipboard {
    pub type_id: Option<&'static str>,
    pub fields: Vec<(&'static str, FieldValue)>,
}

impl ComponentClipboard {
    pub fn set(&mut self, type_id: &'static str, fields: Vec<(&'static str, FieldValue)>) {
        self.type_id = Some(type_id);
        self.fields = fields;
    }

    pub fn matches(&self, type_id: &'static str) -> bool {
        self.type_id == Some(type_id)
    }

    pub fn is_empty(&self) -> bool {
        self.type_id.is_none()
    }
}
