//! ABI-stable types for plugin communication.
//!
//! These types are designed for cross-plugin compatibility.

/// Plugin manifest containing metadata about a plugin.
#[derive(Clone, Debug, Default)]
pub struct PluginManifest {
    /// Unique plugin identifier (e.g., "com.example.my-plugin")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Semantic version
    pub version: String,
    /// Plugin author
    pub author: String,
    /// Description
    pub description: String,
    /// Plugin capabilities
    pub capabilities: Vec<PluginCapability>,
    /// Dependencies on other plugins
    pub dependencies: Vec<PluginDependency>,
    /// Minimum editor API version required
    pub min_api_version: u32,
}

impl PluginManifest {
    /// Create a new plugin manifest
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            author: String::new(),
            description: String::new(),
            capabilities: Vec::new(),
            dependencies: Vec::new(),
            min_api_version: 1,
        }
    }

    /// Set the author
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = author.into();
        self
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Add a capability
    pub fn capability(mut self, cap: PluginCapability) -> Self {
        self.capabilities.push(cap);
        self
    }

    /// Add a dependency
    pub fn dependency(mut self, dep: PluginDependency) -> Self {
        self.dependencies.push(dep);
        self
    }
}

/// Plugin capabilities
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginCapability {
    /// Can register and execute scripts
    ScriptEngine,
    /// Can provide custom gizmos
    Gizmo,
    /// Can register new node types
    NodeType,
    /// Can provide custom inspector widgets
    Inspector,
    /// Can add panels to the editor
    Panel,
    /// Can add menu items
    MenuItem,
    /// Can import asset types
    AssetImporter,
    /// Custom capability
    Custom(String),
}

/// Plugin dependency
#[derive(Clone, Debug)]
pub struct PluginDependency {
    /// ID of the required plugin
    pub plugin_id: String,
    /// Minimum version required
    pub min_version: String,
    /// Whether this dependency is optional
    pub optional: bool,
}

impl PluginDependency {
    /// Create a required dependency
    pub fn required(plugin_id: impl Into<String>, min_version: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            min_version: min_version.into(),
            optional: false,
        }
    }

    /// Create an optional dependency
    pub fn optional(plugin_id: impl Into<String>, min_version: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            min_version: min_version.into(),
            optional: true,
        }
    }
}

/// Error types for plugin operations
#[derive(Clone, Debug)]
pub enum PluginError {
    /// Failed to load the plugin library
    LoadFailed(String),
    /// Plugin initialization failed
    InitFailed(String),
    /// Missing required dependency
    MissingDependency { plugin: String, dependency: String },
    /// Circular dependency detected
    CircularDependency(String),
    /// API version mismatch
    ApiVersionMismatch { required: u32, available: u32 },
    /// Plugin not found
    NotFound(String),
    /// Generic error
    Other(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginError::LoadFailed(msg) => write!(f, "Failed to load plugin: {}", msg),
            PluginError::InitFailed(msg) => write!(f, "Plugin initialization failed: {}", msg),
            PluginError::MissingDependency { plugin, dependency } => {
                write!(f, "Plugin '{}' requires '{}' which is not available", plugin, dependency)
            }
            PluginError::CircularDependency(plugin) => {
                write!(f, "Circular dependency detected involving '{}'", plugin)
            }
            PluginError::ApiVersionMismatch { required, available } => {
                write!(f, "API version mismatch: plugin requires {}, editor provides {}", required, available)
            }
            PluginError::NotFound(id) => write!(f, "Plugin '{}' not found", id),
            PluginError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for PluginError {}

/// Current API version
pub const EDITOR_API_VERSION: u32 = 1;

/// Entity ID for cross-plugin communication
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(C)]
pub struct EntityId(pub u64);

impl EntityId {
    pub const INVALID: Self = Self(u64::MAX);

    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn is_valid(&self) -> bool {
        *self != Self::INVALID
    }
}

impl From<bevy::prelude::Entity> for EntityId {
    fn from(entity: bevy::prelude::Entity) -> Self {
        Self(entity.to_bits())
    }
}

impl EntityId {
    pub fn to_bevy(&self) -> Option<bevy::prelude::Entity> {
        if self.is_valid() {
            Some(bevy::prelude::Entity::from_bits(self.0))
        } else {
            None
        }
    }
}

/// Transform for cross-plugin communication
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct PluginTransform {
    pub translation: [f32; 3],
    pub rotation: [f32; 4], // Quaternion as [x, y, z, w]
    pub scale: [f32; 3],
}

impl From<bevy::prelude::Transform> for PluginTransform {
    fn from(t: bevy::prelude::Transform) -> Self {
        Self {
            translation: t.translation.to_array(),
            rotation: t.rotation.to_array(),
            scale: t.scale.to_array(),
        }
    }
}

impl From<PluginTransform> for bevy::prelude::Transform {
    fn from(t: PluginTransform) -> Self {
        Self {
            translation: bevy::prelude::Vec3::from_array(t.translation),
            rotation: bevy::prelude::Quat::from_array(t.rotation),
            scale: bevy::prelude::Vec3::from_array(t.scale),
        }
    }
}

/// Asset handle for cross-plugin communication
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(C)]
pub struct AssetHandle(pub u64);

impl AssetHandle {
    pub const INVALID: Self = Self(u64::MAX);

    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn is_valid(&self) -> bool {
        *self != Self::INVALID
    }
}

/// Asset loading status
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum AssetStatus {
    /// Asset is being loaded
    Loading,
    /// Asset is loaded and ready
    Loaded,
    /// Asset failed to load
    Failed,
    /// Asset handle is invalid
    Invalid,
}
