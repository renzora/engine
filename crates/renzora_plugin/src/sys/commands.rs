//! Deferred structural changes, and the service mechanism that carries a
//! domain's payload without this module ever learning the domain.

use super::{AssetHandle, ComponentId, Entity, Transform};

/// What a queued command does.
/// Newtype rather than an `enum`, and that is a soundness requirement rather
/// than a style choice.
///
/// The **plugin writes this value and the host reads it** — out of plugin memory
/// for the ones that live in structs, and straight off the FFI boundary for the
/// ones passed by value. Materialising an out-of-range discriminant into a Rust
/// enum is undefined behaviour, and not the harmless kind: rustc attaches
/// `!range` metadata to the load, so LLVM may legally assume the impossible and
/// a `match` can take an arbitrary arm.
///
/// That is exactly what a MINOR bump would cause. A plugin built against a newer
/// ABI writes a discriminant the older host has no variant for. The version
/// handshake is supposed to refuse that plugin — but then the soundness of every
/// appended variant rests on the handshake being bug-free, forever. A newtype
/// removes the question: any `u32` is a valid value, unknown ones fall to the
/// `_` arm, and "appending a variant is a MINOR change" is true rather than
/// merely usually true.
///
/// The constants below keep the variant names, so this is a source-compatible
/// change at every call site.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandKind(pub u32);

#[allow(non_upper_case_globals)]
impl CommandKind {
    /// Despawn `entity` and its descendants.
    pub const Despawn: Self = Self(0);
    /// Insert `component` on `entity`, copying `data_len` bytes from `data`.
    pub const Insert: Self = Self(1);
    /// Remove `component` from `entity`.
    pub const Remove: Self = Self(2);
    /// Make `entity` renderable: the host attaches the real `Mesh3d`,
    /// `MeshMaterial3d` and `Transform`.
    ///
    /// A dedicated command rather than exposing `Mesh3d` as a mirrored
    /// component, because a plugin has no `Handle` type and translating one
    /// through a raw byte copy would mean teaching the insert path about every
    /// host type that contains a handle. `data` holds a [`SpawnMeshDesc`].
    pub const SpawnMesh: Self = Self(3);
    /// Spawn an entity tree described in BSN. `data` is the UTF-8 source.
    ///
    /// The text names components rather than carrying their bytes, which is what
    /// lets one command construct **both** engine components (resolved through
    /// reflection) and the plugin's own (resolved through the field schema the
    /// host already holds for them). A plugin therefore needs no mirror for
    /// `Node`, `PointLight` or anything else it only ever constructs.
    ///
    /// `entity` is a reserved id used as the root of the **first** tree in the
    /// source; any further top-level entries spawn fresh. That is what lets
    /// `spawn_bsn` hand back a usable id in the same frame.
    pub const SpawnBsn: Self = Self(4);
    /// Hand an opaque payload to whichever engine crate claims a service.
    /// `data` is a [`ServiceCall`] header followed by the payload bytes.
    ///
    /// **This is how a domain reaches plugins without this crate learning the
    /// domain.** Animation, audio, physics and navigation are all things a
    /// plugin wants to *do*, and none of them can be a field write — "play this
    /// clip" retargets an animation graph. The obvious design gives each one its
    /// own command kind and its own structs here, and it is the wrong one: this
    /// module is the frozen mechanism, and it would accumulate a vocabulary per
    /// domain, with every domain addition bumping the ABI for plugins that use
    /// none of them.
    ///
    /// So the host carries bytes it does not interpret. A service names itself
    /// with [`service_id`], the payload layout is an agreement between the
    /// plugin and whoever drains the queue, and adding a whole new domain
    /// touches nothing here.
    ///
    /// A command rather than an [`Interface`] function because these are
    /// **structural** in the same sense a spawn is, and because deferring lets a
    /// plugin call one from inside a loop over its own query — which is where
    /// you actually decide to play something.
    ///
    /// Nothing draining a service is a valid configuration: the calls are
    /// discarded each frame, which is the right outcome for a dedicated server
    /// or a lean export that dropped the crate in question.
    ///
    /// [`Interface`]: super::Interface
    pub const Service: Self = Self(5);
    /// Replace `entity`'s material, leaving its mesh and transform alone.
    /// `data` holds a [`SpawnMeshDesc`], of which only `material` is read.
    ///
    /// [`SpawnMesh`](Self::SpawnMesh) could not do this. It sets all three of
    /// mesh, material and transform, so a plugin wanting to shade geometry
    /// somebody else authored had to supply a mesh it did not have — and could
    /// not get, since `Mesh3d` is deliberately opaque and hands back no handle
    /// to pass through. The result was that a plugin material only ever worked
    /// on shapes the plugin spawned itself, which is most of the way to useless:
    /// the interesting case is a custom shader on an imported model.
    ///
    /// Reuses `SpawnMeshDesc` rather than defining a one-field struct, so the
    /// two commands stay obviously related and there is one less layout to keep
    /// frozen forever.
    pub const SetMaterial: Self = Self(6);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 7
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Despawn",
            1 => "Insert",
            2 => "Remove",
            3 => "SpawnMesh",
            4 => "SpawnBsn",
            5 => "Service",
            6 => "SetMaterial",
            _ => "?",
        }
    }
}

// ── Services ─────────────────────────────────────────────────────────────────

/// FNV-1a over a string, `const` so an id folds to a literal at the call site.
///
/// Generic on purpose — this is a hashing primitive, not a service registry.
/// The mechanism here never enumerates services, and adding one is not a change
/// to this file.
pub const fn fnv1a(s: &str) -> u64 {
    let bytes = s.as_bytes();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        h ^= bytes[i] as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    h
}

/// Names a service. Use a dotted, owner-qualified string —
/// `service_id("renzora.animation")` — so two crates cannot collide by both
/// picking `"audio"`.
pub const fn service_id(name: &str) -> u64 {
    fnv1a(name)
}

/// Header for a [`CommandKind::Service`] payload.
///
/// The payload follows this struct in the same `data` buffer, so a service
/// defines its own argument layout without this module knowing any of them. The
/// sink deep-copies `data`, which means a payload must be plain-old-data: a
/// pointer inside it would survive the copy as a pointer and be read after the
/// system returned.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ServiceCall {
    /// From [`service_id`].
    pub service: u64,
    /// Which operation, in the service's own numbering.
    pub op: u32,
    /// Keeps the payload that follows 8-byte aligned.
    pub _pad: u32,
}

impl core::fmt::Debug for CommandKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "CommandKind({})", self.0)
        }
    }
}

/// Payload for [`CommandKind::SpawnMesh`].
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SpawnMeshDesc {
    pub mesh: AssetHandle,
    pub material: AssetHandle,
    pub transform: Transform,
}

/// One queued structural change.
///
/// `data` is only read during the [`CommandSink::push`] call — the host copies
/// what it needs, so a plugin may point at a stack local.
#[repr(C)]
pub struct Command {
    pub kind: CommandKind,
    pub entity: Entity,
    pub component: ComponentId,
    pub data: *const u8,
    pub data_len: usize,
}

/// Structural changes for one system invocation.
///
/// A separate object rather than more [`Interface`] functions, because those
/// take a `Host` handle that is only valid during plugin init — the world is
/// borrowed by the query while a system runs, so there is nothing safe for such
/// a handle to point at. The sink is created per call and dies with it.
///
/// Commands are deferred and applied after the system, exactly like Bevy's:
/// spawning mid-iteration would invalidate the very rows being walked.
///
/// [`Interface`]: super::Interface
#[repr(C)]
pub struct CommandSink {
    /// Allocate an entity id usable immediately, even though the entity does not
    /// exist until commands are applied. Mirrors `Commands::spawn_empty`.
    pub reserve_entity: unsafe extern "C" fn(sink: *mut CommandSink) -> Entity,
    pub push: unsafe extern "C" fn(sink: *mut CommandSink, cmd: *const Command),
}

/// Severity for [`Interface::log`].
/// Newtype rather than an `enum`, and that is a soundness requirement rather
/// than a style choice.
///
/// The **plugin writes this value and the host reads it** — out of plugin memory
/// for the ones that live in structs, and straight off the FFI boundary for the
/// ones passed by value. Materialising an out-of-range discriminant into a Rust
/// enum is undefined behaviour, and not the harmless kind: rustc attaches
/// `!range` metadata to the load, so LLVM may legally assume the impossible and
/// a `match` can take an arbitrary arm.
///
/// That is exactly what a MINOR bump would cause. A plugin built against a newer
/// ABI writes a discriminant the older host has no variant for. The version
/// handshake is supposed to refuse that plugin — but then the soundness of every
/// appended variant rests on the handshake being bug-free, forever. A newtype
/// removes the question: any `u32` is a valid value, unknown ones fall to the
/// `_` arm, and "appending a variant is a MINOR change" is true rather than
/// merely usually true.
///
/// The constants below keep the variant names, so this is a source-compatible
/// change at every call site.
///
/// [`Interface::log`]: super::Interface::log
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LogLevel(pub u32);

#[allow(non_upper_case_globals)]
impl LogLevel {
    pub const Trace: Self = Self(0);
    pub const Debug: Self = Self(1);
    pub const Info: Self = Self(2);
    pub const Warn: Self = Self(3);
    pub const Error: Self = Self(4);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 5
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Trace",
            1 => "Debug",
            2 => "Info",
            3 => "Warn",
            4 => "Error",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for LogLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "LogLevel({})", self.0)
        }
    }
}
