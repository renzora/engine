//! The font database `<text>` is shaped against.
//!
//! # Why never system fonts
//!
//! An SVG with text would otherwise rasterize differently on every machine:
//! Arial on one, Liberation Sans on another, nothing at all in a browser tab or
//! a container. A texture that changes with the machine it was loaded on is a
//! bug report waiting to happen, so the database holds exactly two things:
//!
//! 1. **Noto Sans**, embedded in this crate, which also stands in for every
//!    generic family (`serif`, `sans-serif`, `monospace`, ...) and is the final
//!    fallback. usvg falls back to the `serif` generic for a family it cannot
//!    find, so mapping that generic here is what makes a missing font render
//!    in Noto Sans rather than not at all.
//! 2. **Project fonts**: `.ttf`/`.otf` files named in the sidecar's `fonts`
//!    list, or found by family name under `fonts/` or beside the SVG.
//!
//! Glyphs no font in the database covers (CJK, emoji, unless the project
//! supplies a font for them) are skipped, which is usvg's behaviour too.
//!
//! The bundled copy is this crate's own (the same file `renzora_ember` and
//! `renzora_shell` embed): a runtime crate cannot depend on the UI library to
//! borrow its bytes, and the contract crate stays free of binary assets.

use std::sync::{Arc, OnceLock};

use resvg::usvg::fontdb;

/// Embedded Noto Sans Regular (SIL Open Font License 1.1). The OFL requires
/// its text to travel with the font, so `embedded/OFL.txt` sits beside it and
/// must move with it if the file is ever relocated or replaced.
const NOTO_SANS: &[u8] = include_bytes!("../embedded/NotoSans-Regular.ttf");

/// Family name the bundled font declares, and the default for unstyled text.
pub const DEFAULT_FAMILY: &str = "Noto Sans";

/// The shared base database: built once, cloned only when a load adds
/// project fonts (`Arc::make_mut`), so an SVG without text costs nothing.
pub fn base_database() -> Arc<fontdb::Database> {
    static BASE: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    BASE.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_font_source(fontdb::Source::Binary(Arc::new(NOTO_SANS)));
        db.set_serif_family(DEFAULT_FAMILY);
        db.set_sans_serif_family(DEFAULT_FAMILY);
        db.set_monospace_family(DEFAULT_FAMILY);
        db.set_cursive_family(DEFAULT_FAMILY);
        db.set_fantasy_family(DEFAULT_FAMILY);
        Arc::new(db)
    })
    .clone()
}

/// Whether `family` is already answered by the database, so no file needs to
/// be probed for it. Case-insensitive, as CSS family matching is.
pub fn is_known_family(db: &fontdb::Database, family: &str) -> bool {
    crate::refs::GENERIC_FAMILIES
        .iter()
        .any(|g| g.eq_ignore_ascii_case(family))
        || db.faces().any(|face| {
            face.families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(family))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_font_is_loaded_and_named() {
        let db = base_database();
        assert!(db.len() >= 1);
        assert!(is_known_family(&db, "noto sans"));
        assert!(is_known_family(&db, "serif"));
        assert!(!is_known_family(&db, "Comic Sans MS"));
    }
}
