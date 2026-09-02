//! Translation helpers for the settings overlay.
//!
//! The sidebar's `CATS` const and the enum `label()` methods both store English
//! identities — those strings are load-bearing (they key the group/category
//! match and the dropdown index lookup), so translation happens at *display*
//! time rather than in the data. These functions are that display step.

/// Short alias for the global translation lookup — `tr("key")` → localized
/// `String`. Named `tr` (not `t`) to avoid colliding with the many `let t = …`
/// toggle-entity locals throughout the tab builders.
pub(crate) fn tr(key: &str) -> String {
    renzora::lang::t(key)
}

/// Slugify an option label for the shared `opt.<slug>` translation namespace:
/// lowercase, each run of non-alphanumerics → one `_`, trimmed.
fn opt_slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_us = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_us && !out.is_empty() {
                out.push('_');
            }
            pending_us = false;
            out.push(c.to_ascii_lowercase());
        } else {
            pending_us = true;
        }
    }
    out
}

/// Localize a dropdown OPTION label via the shared `opt.<slug>` namespace,
/// reusing a few `common.*` keys where the value already has one. The enum's
/// `label()` identity (used for index/state matching) is unchanged — only the
/// displayed string is translated.
pub(crate) fn loc_opt(s: &str) -> String {
    match s {
        "None" => renzora::lang::t_or("common.none", s),
        "Disabled" => renzora::lang::t_or("common.disabled", s),
        "Default" => renzora::lang::t_or("common.default", s),
        "Always" => renzora::lang::t_or("common.always", s),
        _ => renzora::lang::t_or(&format!("opt.{}", opt_slug(s)), s),
    }
}

/// Localize a sidebar GROUP header by its English identity (the `CATS` const
/// stores English; translation happens at display so the const stays static).
pub(crate) fn tr_group(group: &str) -> String {
    let key = match group {
        "PROJECT" => "settings.group.project",
        "APPEARANCE" => "settings.group.appearance",
        "EDITOR" => "settings.group.editor",
        "CONTROLS" => "settings.group.controls",
        "PLUGINS" => "settings.group.plugins",
        _ => return group.to_string(),
    };
    tr(key)
}

/// Localize a sidebar CATEGORY label by its English identity (see [`tr_group`]).
pub(crate) fn tr_cat(label: &str) -> String {
    let key = match label {
        "Project" => "common.project",
        "Window" => "settings.cat.window",
        "Rendering" => "settings.cat.rendering",
        "Interface" => "settings.category.interface",
        "Theme" => "settings.tab.theme",
        "General" => "settings.tab.general",
        "Auto-Save" => "settings.cat.autosave",
        "Viewport" => "settings.tab.viewport",
        "Camera" => "settings.category.camera",
        "Gizmos" => "settings.cat.gizmos",
        "Scripting" => "settings.category.scripting",
        "Input" => "settings.cat.input",
        "Shortcuts" => "settings.tab.shortcuts",
        _ => return label.to_string(),
    };
    tr(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── option-label slugs ───────────────────────────────────────────────────

    /// The slug is a translation-table key (`opt.<slug>`), so it has to be
    /// stable and canonical: two labels differing only in punctuation or case
    /// must reach the same key, or half the dropdown silently falls back to
    /// English.
    #[test]
    fn slugs_lowercase_and_collapse_separators() {
        assert_eq!(opt_slug("Screen Space"), "screen_space");
        assert_eq!(opt_slug("SCREEN SPACE"), "screen_space");
        assert_eq!(opt_slug("Screen-Space"), "screen_space");
        assert_eq!(opt_slug("Screen  ---  Space"), "screen_space");
        assert_eq!(opt_slug("Anti-Aliasing (TAA)"), "anti_aliasing_taa");
    }

    /// A run of separators at either end must not leave a dangling underscore —
    /// `opt.screen_space_` and `opt.screen_space` are different keys and only one
    /// of them is in the table.
    #[test]
    fn slugs_have_no_leading_or_trailing_underscore() {
        for label in ["  Screen Space  ", "(Screen Space)", "- Screen Space -", "Screen Space!"] {
            let slug = opt_slug(label);
            assert!(!slug.starts_with('_'), "{label:?} -> {slug:?}");
            assert!(!slug.ends_with('_'), "{label:?} -> {slug:?}");
            assert_eq!(slug, "screen_space", "{label:?}");
        }
    }

    #[test]
    fn slugs_keep_digits() {
        assert_eq!(opt_slug("MSAA 4x"), "msaa_4x");
        assert_eq!(opt_slug("2048"), "2048");
    }

    #[test]
    fn a_label_with_nothing_alphanumeric_slugs_to_nothing() {
        assert_eq!(opt_slug("---"), "");
        assert_eq!(opt_slug(""), "");
    }

    // ── localization fallbacks ───────────────────────────────────────────────

    /// Every one of these must come back non-empty. `t_or` falls back to the
    /// supplied English, so an empty result means a blank dropdown row or a
    /// blank sidebar header — which reads as a broken UI rather than as a
    /// missing translation.
    #[test]
    fn localization_never_returns_an_empty_label() {
        for label in ["None", "Disabled", "Default", "Always", "Screen Space", "Anything Else"] {
            assert!(!loc_opt(label).is_empty(), "loc_opt({label:?}) was empty");
        }
        for group in ["PROJECT", "APPEARANCE", "EDITOR", "CONTROLS", "PLUGINS", "SOMETHING NEW"] {
            assert!(!tr_group(group).is_empty(), "tr_group({group:?}) was empty");
        }
        for cat in ["Project", "Window", "Rendering", "Interface", "Theme", "Unmapped Category"] {
            assert!(!tr_cat(cat).is_empty(), "tr_cat({cat:?}) was empty");
        }
    }

    /// An unmapped group or category passes through verbatim rather than
    /// becoming a key that does not exist — that is what lets a plugin add a
    /// sidebar group without also shipping a translation.
    #[test]
    fn unmapped_groups_and_categories_pass_through_unchanged() {
        assert_eq!(tr_group("MY PLUGIN"), "MY PLUGIN");
        assert_eq!(tr_cat("My Plugin Settings"), "My Plugin Settings");
    }
}
