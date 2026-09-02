//! Resolving component ids during `Plugin::build`.
//!
//! A plugin naming a component the host does not expose must **refuse to
//! load**. Carrying on would register a system whose query can never match,
//! which presents as "my plugin loaded but does nothing" — far harder to
//! diagnose than a refusal at startup. That is what `unresolved` is for.

use crate::sys;

use super::component::Component;
use super::resource::Resource;

/// Resolves component ids during `Plugin::build`, caching so a type is
/// registered or looked up once regardless of how many systems name it.
pub struct InitCtx {
    pub(crate) iface: *const sys::Interface,
    pub(crate) host: *mut sys::Host,
    pub(crate) cache: alloc::vec::Vec<(&'static str, sys::ComponentId)>,
    /// Type path of the first component that failed to resolve, if any.
    ///
    /// A plugin naming a component the host does not expose must refuse to load.
    /// Carrying on would register a system whose query can never match, which
    /// presents as "my plugin loaded but does nothing" — far harder to diagnose
    /// than a refusal at startup.
    pub(crate) unresolved: Option<&'static str>,
}

impl InitCtx {
    /// Register `T` as a resource and cache the id on the type.
    ///
    /// Registration is idempotent host-side (a second call returns the same id
    /// and leaves the existing value alone), so two systems both taking
    /// `ResMut<Score>` do not reset it between them.
    pub(crate) fn resource_id_of<T: Resource>(&mut self) -> sys::ComponentId {
        // Cached on the `InitCtx`, not on the type. The type's cell is the
        // *runtime* lookup key and is only correct for the world this plugin was
        // loaded into; short-circuiting on it would let one world's id leak into
        // another and silently skip registration there.
        if let Some((_, id)) = self.cache.iter().find(|(p, _)| *p == T::TYPE_PATH) {
            return *id;
        }
        let desc = T::descriptor();
        // SAFETY: `iface`/`host` are valid for the whole init call.
        let id = unsafe { ((*self.iface).register_resource)(self.host, &desc) };
        if !id.is_valid() && self.unresolved.is_none() {
            self.unresolved = Some(T::TYPE_PATH);
        }
        self.send_ranges(id, T::field_ranges());
        T::id_cell().store(id.0, core::sync::atomic::Ordering::Relaxed);
        self.cache.push((T::TYPE_PATH, id));
        id
    }

    pub(crate) fn id_of<T: Component>(&mut self) -> sys::ComponentId {
        if let Some((_, id)) = self.cache.iter().find(|(p, _)| *p == T::TYPE_PATH) {
            return *id;
        }
        // SAFETY: `iface`/`host` are valid for the whole init call.
        let id = unsafe {
            match T::descriptor() {
                Some(desc) => ((*self.iface).register_component)(self.host, &desc),
                None => ((*self.iface).component_id_by_name)(
                    self.host,
                    sys::StrRef::new(T::TYPE_PATH),
                ),
            }
        };
        if !id.is_valid() && self.unresolved.is_none() {
            self.unresolved = Some(T::TYPE_PATH);
        }
        self.send_ranges(id, T::field_ranges());
        // Cache on the type itself, so systems can reach it later.
        T::id_cell().store(id.0, core::sync::atomic::Ordering::Relaxed);
        self.cache.push((T::TYPE_PATH, id));
        id
    }

    /// Send each field's editing range, one call per ranged field.
    ///
    /// Silently skipped on a host older than MINOR 3, which has no
    /// `set_field_range` in its table — reading past the end of the interface a
    /// host actually published would be exactly the bug the version handshake
    /// exists to prevent. A plugin on an old host gets unbounded drags, which is
    /// what that host did for everything anyway.
    fn send_ranges(&mut self, id: sys::ComponentId, ranges: &'static [(usize, sys::FieldRange)]) {
        if ranges.is_empty() || !id.is_valid() {
            return;
        }
        // SAFETY: `iface` points at the host's table, valid for the init call.
        let minor = unsafe { (*self.iface).version_minor };
        if minor < 3 {
            return;
        }
        for (index, range) in ranges {
            // SAFETY: same lifetime as every other init-time call; `range` outlives
            // it, being a `'static`.
            unsafe { ((*self.iface).set_field_range)(self.host, id, *index, range) };
        }
    }
}

/// The id the host assigned `T`, or `INVALID` if it was never registered.
///
/// Registration happens during `Plugin::build`; a system naming a component the
/// plugin never registered gets `INVALID`, and the host ignores commands
/// carrying it rather than guessing.
pub fn component_id_of<T: Component>() -> sys::ComponentId {
    sys::ComponentId(T::id_cell().load(core::sync::atomic::Ordering::Relaxed))
}
