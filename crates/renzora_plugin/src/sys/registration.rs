//! What a plugin declares up front: its components and their editable fields,
//! and the access pattern of each system.
//!
//! Declaring access at registration is what lets the host build a real Bevy
//! query, so a plugin system schedules in parallel with anything it does not
//! conflict with — a plugin that could reach arbitrary components on a whim
//! would have to be exclusive, serialising the whole schedule.

use core::ffi::c_void;

use super::{ComponentId, Entity, Schedule, StrRef, SystemEntry};

/// The type of one inspectable field.
///
/// Append-only `#[repr(u32)]`. Deliberately a small closed set rather than
/// anything reflection-shaped: the editor has to render a widget for each, so a
/// kind nobody can draw is worse than no kind at all.
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
pub struct FieldKind(pub u32);

#[allow(non_upper_case_globals)]
impl FieldKind {
    pub const F32: Self = Self(0);
    pub const I32: Self = Self(1);
    pub const Bool: Self = Self(2);
    pub const Vec3: Self = Self(3);
    pub const Quat: Self = Self(4);
    /// A [`Str256`] — fixed-capacity inline UTF-8.
    pub const Str: Self = Self(5);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 6
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "F32",
            1 => "I32",
            2 => "Bool",
            3 => "Vec3",
            4 => "Quat",
            5 => "Str",
            _ => "?",
        }
    }
}

/// Payload bytes in a [`Str256`]. The `+ 4` for the length makes the whole
/// thing 256 bytes.
pub const STR_CAP: usize = 252;

/// A fixed-capacity, inline UTF-8 string a plugin component can hold.
///
/// ## Why not just `String`
///
/// Component storage is allocated by the host from a layout the plugin
/// declares, and a plugin component is refused outright if it declares a
/// destructor — a `String` whose drop never runs leaks its buffer silently for
/// the life of the process. Storage is also read as raw bytes by the query
/// path, the scene serializer and the inspector, none of which can chase a
/// pointer into the plugin's heap and none of which could free it.
///
/// So the bytes live in the component. That costs 256 bytes per field whatever
/// the content, which is the trade: a name, a path or a line of 3D text fits
/// comfortably, and anything genuinely large belongs in an asset rather than a
/// component.
///
/// A single capacity rather than a family of sizes, deliberately. The size has
/// to be recoverable from [`FieldKind`] alone — [`FieldDesc`] is walked as an
/// array at a fixed stride, so growing it to carry a per-field length would be
/// a MAJOR break.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Str256 {
    pub bytes: [u8; STR_CAP],
    /// Bytes used. Always `<= STR_CAP`; every reader clamps rather than
    /// trusting it, because this crosses from another compilation unit.
    pub len: u32,
}

impl Str256 {
    pub const EMPTY: Self = Self { bytes: [0; STR_CAP], len: 0 };

    /// Copy `s` in, or `None` if it does not fit.
    ///
    /// `None` rather than truncating: a silently cut path or name fails to
    /// resolve later, somewhere with no memory of where the string came from.
    pub const fn new(s: &str) -> Option<Self> {
        let src = s.as_bytes();
        if src.len() > STR_CAP {
            return None;
        }
        let mut out = Self::EMPTY;
        let mut i = 0;
        while i < src.len() {
            out.bytes[i] = src[i];
            i += 1;
        }
        out.len = src.len() as u32;
        Some(out)
    }

    /// Copy `s` in, truncating at the last character boundary that fits.
    ///
    /// For the inspector and other interactive paths, where refusing a keystroke
    /// is worse than not accepting the tail of an over-long paste.
    pub fn new_truncating(s: &str) -> Self {
        let mut end = s.len().min(STR_CAP);
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        Self::new(&s[..end]).unwrap_or(Self::EMPTY)
    }

    /// The bytes in use, clamped to the capacity.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..(self.len as usize).min(STR_CAP)]
    }

    /// The contents, or `""` if a plugin wrote bytes that are not UTF-8.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }
}

impl Default for Str256 {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl core::fmt::Debug for Str256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl core::fmt::Display for Str256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq for Str256 {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl Eq for Str256 {}

impl core::fmt::Debug for FieldKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "FieldKind({})", self.0)
        }
    }
}

/// One editable field of a plugin component.
///
/// This exists because a plugin component has no `TypeRegistration` — the engine
/// knows its size and alignment but nothing about its shape, so the inspector
/// has no way to show it and nothing could ever put one on an entity. The
/// schema is what makes a plugin component *usable* rather than merely storable.
///
/// `offset` is a byte offset into the component, which the plugin gets from
/// `core::mem::offset_of!`. The host reads and writes through it directly.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FieldDesc {
    pub name: StrRef,
    pub kind: FieldKind,
    pub offset: usize,
}

/// Describes a plugin-owned component so the host can register it via
/// `World::register_component_with_descriptor`.
///
/// `drop` is `None` for plain-data components, which is the strongly preferred
/// case: a component with no destructor can be stripped from entities at unload
/// without calling back into a library that is about to be removed from the
/// address space. A component that *does* need a destructor makes the plugin
/// un-unloadable in practice.
#[repr(C)]
pub struct ComponentDesc {
    /// Fully-qualified type path, e.g. `"my_spinner::Spinner"`. This is the
    /// component's identity — it is what scenes serialize and what
    /// [`Interface::component_id_by_name`] matches on, so renaming it breaks
    /// saved scenes exactly like renaming a Rust type would.
    ///
    /// [`Interface::component_id_by_name`]: super::Interface::component_id_by_name
    pub name: StrRef,
    pub size: usize,
    pub align: usize,
    pub drop: Option<unsafe extern "C" fn(*mut u8)>,
    /// Human-readable name for the editor's "Add Component" list. Empty falls
    /// back to the last segment of `name`.
    pub display_name: StrRef,
    /// Inspectable fields. May be empty — a marker component has none, and it is
    /// still addable, just with nothing to edit.
    pub fields: *const FieldDesc,
    pub field_count: usize,
    /// Writes one default-valued instance into `size` bytes of host-provided
    /// storage. Used when the editor adds the component to an entity.
    ///
    /// A function rather than a pointer to a default instance: a derive cannot
    /// build a `static` of an arbitrary user type (that needs const
    /// construction), and a pointer into a temporary would dangle the moment
    /// `descriptor()` returned. `None` falls back to zeroed memory, which is
    /// wrong for anything whose sensible default isn't all-zero — a scale of 0,
    /// a speed of 0.
    pub default_init: Option<unsafe extern "C" fn(*mut u8)>,
}

/// How a query term touches its component.
///
/// `#[repr(u32)]` and append-only, like [`Schedule`]. Filter terms (`With` /
/// `Without`) are not just sugar — without them a plugin can only express "every
/// entity that has a Transform", which in a real scene includes the editor
/// camera and every light. Filtering is how a plugin scopes itself to the
/// entities it actually owns.
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
pub struct Access(pub u32);

#[allow(non_upper_case_globals)]
impl Access {
    /// Read-only data access. Produces a cell; never written back.
    pub const Read: Self = Self(0);
    /// Mutable data access. Produces a cell, and the host copies it back after
    /// the call.
    pub const Write: Self = Self(1);
    /// Filter only — the entity must have this component. No cell is produced,
    /// so the plugin must not count it when indexing [`SystemCall::cells`].
    ///
    /// [`SystemCall::cells`]: super::SystemCall::cells
    pub const With: Self = Self(2);
    /// Filter only — the entity must NOT have this component.
    pub const Without: Self = Self(3);
    /// Read-only access to a **resource**, not a component. Contributes no cell
    /// and no filtering; the host resolves it once per call into
    /// [`SystemCall::resources`].
    ///
    /// [`SystemCall::resources`]: super::SystemCall::resources
    pub const ResRead: Self = Self(4);
    /// Mutable resource access.
    pub const ResWrite: Self = Self(5);
    /// Optional component data. Produces a cell like [`Access::Read`], but the
    /// cell is **null** when the entity lacks the component, and the entity
    /// still matches. Mirrors Bevy's `Option<&T>`.
    pub const ReadOptional: Self = Self(6);
    /// Optional mutable component data. Mirrors `Option<&mut T>`.
    pub const WriteOptional: Self = Self(7);
    /// Opens an `Or` group. Carries no component.
    pub const OrBegin: Self = Self(8);
    /// Separates one `Or` branch from the next. Carries no component.
    pub const OrNext: Self = Self(9);
    /// Closes an `Or` group. Carries no component.
    pub const OrEnd: Self = Self(10);
    /// Match only rows whose component was added since this system last ran.
    /// Carries the component but produces **no cell** — it is a filter.
    pub const Added: Self = Self(11);
    /// Match only rows whose component changed since this system last ran.
    pub const Changed: Self = Self(12);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 13
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Read",
            1 => "Write",
            2 => "With",
            3 => "Without",
            4 => "ResRead",
            5 => "ResWrite",
            6 => "ReadOptional",
            7 => "WriteOptional",
            8 => "OrBegin",
            9 => "OrNext",
            10 => "OrEnd",
            11 => "Added",
            12 => "Changed",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for Access {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "Access({})", self.0)
        }
    }
}

impl Access {
    /// Whether this term is a grouping marker rather than a component or
    /// resource reference. Markers carry [`ComponentId::INVALID`].
    pub const fn is_marker(self) -> bool {
        matches!(self, Access::OrBegin | Access::OrNext | Access::OrEnd)
    }

    /// Whether this term contributes a cell to [`SystemCall::cells`]. Filter and
    /// resource terms do not, which is why cell indices are *not* term indices.
    ///
    /// [`SystemCall::cells`]: super::SystemCall::cells
    pub const fn has_cell(self) -> bool {
        matches!(
            self,
            Access::Read | Access::Write | Access::ReadOptional | Access::WriteOptional
        )
    }

    /// Whether this term names a resource rather than a component.
    pub const fn is_resource(self) -> bool {
        matches!(self, Access::ResRead | Access::ResWrite)
    }
}

/// One component a system's query touches.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Term {
    pub component: ComponentId,
    pub access: Access,
}

impl Term {
    /// A grouping marker — an `Or` bracket, which names no component.
    pub const fn marker(access: Access) -> Self {
        Self {
            component: ComponentId::INVALID,
            access,
        }
    }
}

/// The full access pattern of one system, declared up front at registration.
///
/// The host turns this into a real Bevy query via `QueryParamBuilder`, so the
/// resulting system carries proper component access and **schedules in parallel**
/// with anything it does not conflict with. Declaring access up front is what
/// buys that — a plugin that could reach arbitrary components on a whim would
/// have to be an exclusive system, serialising the whole schedule.
#[repr(C)]
pub struct QueryDesc {
    pub terms: *const Term,
    pub term_count: usize,
}

/// One query's matched rows, for one call.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct QueryView {
    /// Row-major `entity_count × cell_count`.
    pub cells: *mut *mut u8,
    pub entities: *const Entity,
    pub entity_count: usize,
    /// Cells per row — the count of [`Access::has_cell`] terms, which is NOT the
    /// number of terms once filters are involved.
    pub cell_count: usize,
}

/// Everything about one system, registered in a single call.
///
/// A struct rather than a parameter list because this is the part of the ABI
/// most likely to grow — run conditions, ordering labels and system sets all
/// land here — and a struct the plugin *writes* can only grow at the end. That
/// is also why [`flags`](Self::flags) exists now while nothing sets it: a
/// reserved word costs four bytes today and saves a MAJOR bump later.
#[repr(C)]
pub struct SystemDesc {
    pub entry: SystemEntry,
    pub schedule: Schedule,
    /// Queries, in the order the system's parameters declare them. The host
    /// hands back one [`QueryView`] per entry, in the same order.
    pub queries: *const QueryDesc,
    pub query_count: usize,
    /// Resources the system touches, as [`Access::ResRead`] / [`Access::ResWrite`]
    /// terms. Separate from the queries because a resource is per-system, not
    /// per-query.
    pub resources: *const Term,
    pub resource_count: usize,
    /// Opaque value handed back in [`SystemCall::user`].
    ///
    /// [`SystemCall::user`]: super::SystemCall::user
    pub user: *mut c_void,
    /// Reserved. Must be zero.
    pub flags: u32,
}

/// Why [`Interface::add_system`] refused a system.
///
/// `add_system` used to return nothing, and it runs inside the host's panic
/// guard — so Bevy's access-conflict panic and a failed term resolution were
/// both caught, logged, and swallowed. The plugin's `init` returned `Ok` with a
/// system silently missing, which presents as "my plugin loaded and does
/// nothing": the single hardest failure to diagnose from the outside.
/// Newtype rather than an `enum`, and this one crosses in the **opposite**
/// direction to most: the host produces it, the plugin materialises it.
///
/// That direction is the more dangerous of the two, and the rule as originally
/// written did not cover it — it was phrased entirely as "plugin writes, host
/// reads". The handshake **deliberately accepts a newer host**, because the table
/// is append-only. So a host that adds a fifth status hands every
/// already-compiled plugin a discriminant it has no variant for, and the plugin
/// materialises it where the host's panic guard cannot see the consequences.
///
/// The pressure to append is not hypothetical: the host already detects a
/// duplicate panel id and has to report the generic `Invalid` for it.
///
/// [`Interface::add_system`]: super::Interface::add_system
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RegisterStatus(pub u32);

#[allow(non_upper_case_globals)]
impl RegisterStatus {
    pub const Ok: Self = Self(0);
    /// A term named a component id the host does not have.
    pub const UnknownComponent: Self = Self(1);
    /// Bevy refused the access pattern — usually `&mut T` and `&T` on the same
    /// component in one system.
    pub const AccessConflict: Self = Self(2);
    /// The descriptor was malformed: a null pointer, no queries, or a non-zero
    /// `flags`.
    pub const Invalid: Self = Self(3);

    /// Whether this is a value this build knows. A plugin should treat anything
    /// else as a failure — the host is newer and refused for a reason this build
    /// has no name for.
    pub const fn is_known(self) -> bool {
        self.0 < 4
    }
}
