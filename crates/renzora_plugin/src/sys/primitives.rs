//! The scalar types every other part of the boundary is built from, and the
//! frozen mirrors of the host's own math types.
//!
//! Everything here is embedded **by value** in other boundary structs, which is
//! the trap the module docs in [`super`] warn about: growing [`StrRef`] or
//! [`Vec3`] is an ABI break authored in a different file from the struct it
//! breaks. Grep for a type as a field before touching it.

/// An entity, as Bevy's `Entity::to_bits()`. Opaque to the plugin — only ever
/// handed back to the host.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Entity(pub u64);

impl Entity {
    /// An entity that is deliberately no entity.
    ///
    /// For service calls that are not about anything in the world — an HTTP
    /// request belongs to the plugin, not to a body — so the consumer still
    /// sees one shape whether or not the call has a subject. `u64::MAX` never
    /// round-trips through `Entity::try_from_bits`, so it cannot be mistaken
    /// for a real handle.
    pub const PLACEHOLDER: Self = Self(u64::MAX);
}

/// A component's runtime id, assigned by the host. Resolved either by
/// registering a plugin-owned component ([`Interface::register_component`]) or
/// by looking up a host one ([`Interface::component_id_by_name`]).
///
/// [`Interface::register_component`]: super::Interface::register_component
/// [`Interface::component_id_by_name`]: super::Interface::component_id_by_name
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ComponentId(pub u32);

impl ComponentId {
    /// Returned by [`Interface::component_id_by_name`] when no component with
    /// that type path is registered. A plugin that queries on this will never
    /// match anything, so it should fail loudly at registration instead.
    ///
    /// [`Interface::component_id_by_name`]: super::Interface::component_id_by_name
    pub const INVALID: ComponentId = ComponentId(u32::MAX);

    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

/// A borrowed UTF-8 string. Never transfers ownership in either direction — the
/// callee must copy if it wants to keep the bytes. This sidesteps the "whose
/// allocator frees it" problem entirely, which matters because the host and the
/// plugin have separate allocators and may not even share a rustc version.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct StrRef {
    pub ptr: *const u8,
    pub len: usize,
}

// SAFETY: a `StrRef` is a shared borrow of immutable UTF-8 bytes — `Sync` and
// `Send` for exactly the same reason `&'static str` is. The raw pointer is only
// there to keep the type `#[repr(C)]`; nothing ever writes through it. Without
// these, a component's field schema could not live in a `static`, which is the
// only sensible place for it.
unsafe impl Send for StrRef {}
unsafe impl Sync for StrRef {}

impl StrRef {
    pub const fn new(s: &'static str) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    /// # Safety
    /// The bytes must still be alive and valid UTF-8 for `'a`.
    pub unsafe fn as_str<'a>(self) -> &'a str {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(self.ptr, self.len))
    }
}

/// Which of the host's schedules a system runs in. `#[repr(u32)]` so the value
/// is a stable part of the ABI — append new variants, never renumber.
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
pub struct Schedule(pub u32);

#[allow(non_upper_case_globals)]
impl Schedule {
    pub const First: Self = Self(0);
    pub const PreUpdate: Self = Self(1);
    pub const Update: Self = Self(2);
    pub const PostUpdate: Self = Self(3);
    pub const Last: Self = Self(4);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 5
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "First",
            1 => "PreUpdate",
            2 => "Update",
            3 => "PostUpdate",
            4 => "Last",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for Schedule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "Schedule({})", self.0)
        }
    }
}

// ── Frozen host-type mirrors ─────────────────────────────────────────────────
//
// Once published, the layout of everything in this section is frozen forever.
// Adding a field is a MAJOR bump. Keep the list short: every type here is one
// the host has to marshal on every system call.

/// Mirrors `glam::Vec3`. Defined here rather than re-exported because glam's own
/// representation is not a stability promise to us (see the module docs).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Mirrors `glam::Quat` as plain `xyzw` floats — deliberately *not* the SIMD
/// representation, which varies by target.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

/// Mirrors `bevy::Transform`. Field order here is the contract; the host
/// converts to and from the real thing, which is free to lay itself out however
/// the compiler likes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}
