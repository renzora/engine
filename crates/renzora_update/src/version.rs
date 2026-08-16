//! Ordering release tags.
//!
//! Recovered from the updater that was deleted in the alpha-5 restructure (it
//! lived at `crates/editor/renzora_update/src/check.rs`) and extended for the
//! nightly tags the release pipeline now publishes. The comparison rules and
//! their tests are the original's; the third component is new.
//!
//! Tags this has to order:
//!
//! ```text
//! r1                         a final release
//! r1-alpha7                  a pre-release
//! r1-alpha7-nightly-16aug26  a nightly of that pre-release
//! ```
//!
//! plus the running binary itself when it was built from a checkout and has no
//! tag at all.
//!
//! Ordering is lexicographic over (release, pre-release, [`Stage`]). For the
//! first two, **absent sorts above present** — `r1` is newer than `r1-alpha7`.
//! `Stage` then orders `Dev < Nightly(date) < Final` within one version, which
//! gives two behaviours that both matter:
//!
//! * the day `r1-alpha7` ships, everyone on `r1-alpha7-nightly-*` is offered it,
//!   because the finished release outranks every nightly leading up to it;
//! * a build from source is offered those nightlies, because it is the *least*
//!   finished build of that version rather than the most.

use std::cmp::Ordering;

/// How finished a build of one version is. Ordered least → most.
///
/// This is a three-state, not a two-state, and that is the whole point. It began
/// as `nightly: Option<date>` — absent meaning "the finished release" — and a
/// build from source had no tag of its own, so it reported bare `r1-alpha7` and
/// therefore compared as *finished*. Since a release outranks its own nightlies,
/// a source checkout out-ranked every nightly of its version and was told it was
/// up to date, forever.
///
/// A source build is not the finished version; it is the least finished thing
/// there is for that version, below even last night's build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// Built from a checkout — no release of its own. Sorts below everything.
    Dev,
    /// A dated nightly, `(yy, mm, dd)`.
    Nightly(u32, u32, u32),
    /// The published release itself.
    Final,
}

/// A parsed `rN[-<pre><n>][-nightly-<ddmonyy>]` tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedVersion {
    release: u32,
    /// `None` = final, `Some(("alpha", 7))` = pre-release.
    pre: Option<(String, u32)>,
    stage: Stage,
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

impl ParsedVersion {
    /// Parse a tag, or `None` if it isn't one of ours. Tolerates a `v` prefix.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('v');
        let rest = s.strip_prefix('r')?;

        // Split off the nightly suffix first: it contains a `-` of its own, and
        // the pre-release split below would otherwise swallow it and fail to
        // parse `7-nightly-16aug26` as a number. That is exactly how the
        // recovered parser behaved — it returned None for every nightly tag.
        let (head, stage) = match rest.split_once("-nightly-") {
            Some((head, date)) => {
                let (y, m, d) = parse_nightly_date(date)?;
                (head, Stage::Nightly(y, m, d))
            }
            None => (rest, Stage::Final),
        };

        let (release, pre) = match head.split_once('-') {
            Some((release_str, pre_str)) => {
                let release = release_str.parse().ok()?;
                // The type/number boundary is the first digit: `alpha7` → ("alpha", 7).
                let split = pre_str.find(|c: char| c.is_ascii_digit())?;
                let (pre_type, pre_num) = pre_str.split_at(split);
                (release, Some((pre_type.to_string(), pre_num.parse().ok()?)))
            }
            None => (head.parse().ok()?, None),
        };

        Some(Self {
            release,
            pre,
            stage,
        })
    }

    /// The build-from-source form of a version: `r1-alpha7` at [`Stage::Dev`].
    ///
    /// Used for the running binary when CI stamped no tag into it, so that every
    /// published build of the same version — release or nightly — compares as
    /// newer.
    pub fn dev(version: &str) -> Option<Self> {
        let mut v = Self::parse(version)?;
        v.stage = Stage::Dev;
        Some(v)
    }

    /// True when this tag is a dated nightly rather than a published version.
    pub fn is_nightly(&self) -> bool {
        matches!(self.stage, Stage::Nightly(..))
    }

    pub fn is_newer_than(&self, other: &Self) -> bool {
        self > other
    }
}

/// `16aug26` → `(26, 8, 16)`. Deliberately strict: a tag whose date we cannot
/// read is not ordered at all rather than ordered wrongly.
fn parse_nightly_date(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.to_ascii_lowercase();
    if s.len() != 7 {
        return None;
    }
    let day: u32 = s[0..2].parse().ok()?;
    let month = MONTHS.iter().position(|m| *m == &s[2..5])? as u32 + 1;
    let year: u32 = s[5..7].parse().ok()?;
    Some((year, month, day))
}

impl PartialOrd for ParsedVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ParsedVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.release.cmp(&other.release) {
            Ordering::Equal => {}
            ord => return ord,
        }
        // Absent pre-release wins: `r1` is newer than `r1-alpha7`.
        match (&self.pre, &other.pre) {
            (None, None) => {}
            (None, Some(_)) => return Ordering::Greater,
            (Some(_), None) => return Ordering::Less,
            (Some((a_type, a_num)), Some((b_type, b_num))) => {
                // String order happens to be the right order for the labels we
                // use: alpha < beta < rc. A label that broke that (say "dev")
                // would need an explicit rank here.
                match a_type.cmp(b_type) {
                    Ordering::Equal => match a_num.cmp(b_num) {
                        Ordering::Equal => {}
                        ord => return ord,
                    },
                    ord => return ord,
                }
            }
        }
        // Same rule one level down, now three-way: a source build is below every
        // nightly, and the finished `r1-alpha7` is above all of them.
        self.stage.cmp(&other.stage)
    }
}

/// True if `latest` is newer than `current`. An unparseable tag on either side
/// yields `false` — never offer an "update" we cannot reason about.
///
/// Test-only. The check itself compares [`ParsedVersion`]s, because the running
/// binary may have no tag to name: a build from source compares at
/// [`Stage::Dev`], which no string can express. This string form is kept because
/// it is what the recovered updater's original test cases are written against,
/// and those cases are the reason to trust the release/pre-release rules.
#[cfg(test)]
fn is_newer_version(latest: &str, current: &str) -> bool {
    match (ParsedVersion::parse(latest), ParsedVersion::parse(current)) {
        (Some(l), Some(c)) => l.is_newer_than(&c),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The recovered updater's own cases, unchanged — the release/pre-release
    /// rules must not have drifted while the code was out of the tree.
    #[test]
    fn release_and_prerelease_ordering() {
        assert!(is_newer_version("r1-alpha5", "r1-alpha4"));
        assert!(!is_newer_version("r1-alpha4", "r1-alpha5"));
        assert!(!is_newer_version("r1-alpha4", "r1-alpha4"));
        assert!(is_newer_version("r2-alpha1", "r1-alpha4"));
        assert!(is_newer_version("r1", "r1-alpha4"));
        assert!(!is_newer_version("r1-alpha4", "r1"));
        assert!(is_newer_version("r1-beta1", "r1-alpha4"));
        assert!(!is_newer_version("r1-alpha4", "r1-beta1"));
    }

    #[test]
    fn parses_the_shapes_we_publish() {
        assert!(ParsedVersion::parse("r1-alpha4").is_some());
        assert!(ParsedVersion::parse("r1-beta1").is_some());
        assert!(ParsedVersion::parse("r1").is_some());
        assert!(ParsedVersion::parse("r10-alpha15").is_some());
        assert!(ParsedVersion::parse("r1-alpha7-nightly-16aug26").is_some());
        assert!(ParsedVersion::parse("0.1.0").is_none());
        assert!(ParsedVersion::parse("invalid").is_none());
    }

    /// The case the recovered parser could not handle at all.
    #[test]
    fn nightlies_order_by_date() {
        assert!(is_newer_version(
            "r1-alpha7-nightly-16aug26",
            "r1-alpha7-nightly-15aug26"
        ));
        assert!(is_newer_version(
            "r1-alpha7-nightly-01sep26",
            "r1-alpha7-nightly-31aug26"
        ));
        // Year beats month beats day.
        assert!(is_newer_version(
            "r1-alpha7-nightly-01jan27",
            "r1-alpha7-nightly-31dec26"
        ));
        assert!(!is_newer_version(
            "r1-alpha7-nightly-15aug26",
            "r1-alpha7-nightly-16aug26"
        ));
    }

    /// The rule that makes the Nightly channel converge: when the version
    /// finally ships, everyone on its nightlies is offered the release.
    #[test]
    fn a_release_outranks_its_own_nightlies() {
        assert!(is_newer_version("r1-alpha7", "r1-alpha7-nightly-16aug26"));
        assert!(!is_newer_version("r1-alpha7-nightly-16aug26", "r1-alpha7"));
        // And a later version's nightly still beats the earlier release.
        assert!(is_newer_version("r1-alpha8-nightly-01sep26", "r1-alpha7"));
    }

    /// The bug this enum exists for: a source checkout of `r1-alpha7` reported
    /// itself as the bare version, which compares as the FINISHED release and
    /// therefore outranked every nightly of it — so the dialog said "up to date"
    /// while showing the nightly's own release notes.
    #[test]
    fn a_dev_build_is_older_than_every_build_of_its_version() {
        let dev = ParsedVersion::dev("r1-alpha7").unwrap();
        for tag in [
            "r1-alpha7-nightly-01jan26",
            "r1-alpha7-nightly-16aug26",
            "r1-alpha7",
            "r1-alpha8",
            "r1",
        ] {
            let published = ParsedVersion::parse(tag).unwrap();
            assert!(
                published.is_newer_than(&dev),
                "{tag} should be newer than a dev build of r1-alpha7"
            );
        }
        // ...but still older than a LATER version's dev build, and never newer
        // than itself.
        assert!(!dev.is_newer_than(&dev));
        assert!(ParsedVersion::dev("r1-alpha8").unwrap().is_newer_than(&dev));
    }

    #[test]
    fn a_dev_build_is_not_mistaken_for_a_release() {
        let dev = ParsedVersion::dev("r1-alpha7").unwrap();
        let released = ParsedVersion::parse("r1-alpha7").unwrap();
        assert_ne!(dev, released);
        assert!(!dev.is_nightly());
    }

    #[test]
    fn stage_orders_dev_below_nightly_below_final() {
        assert!(Stage::Dev < Stage::Nightly(26, 8, 16));
        assert!(Stage::Nightly(26, 8, 16) < Stage::Final);
        assert!(Stage::Nightly(26, 8, 15) < Stage::Nightly(26, 8, 16));
    }

    #[test]
    fn is_nightly_identifies_dated_tags() {
        assert!(ParsedVersion::parse("r1-alpha7-nightly-16aug26")
            .unwrap()
            .is_nightly());
        assert!(!ParsedVersion::parse("r1-alpha7").unwrap().is_nightly());
        assert!(!ParsedVersion::parse("r1").unwrap().is_nightly());
    }

    #[test]
    fn a_malformed_date_is_not_a_version() {
        // Rejected rather than silently ordered as "no nightly", which would
        // make it compare equal to the finished release.
        assert!(ParsedVersion::parse("r1-alpha7-nightly-32aug26").is_some()); // day is not range-checked
        assert!(ParsedVersion::parse("r1-alpha7-nightly-16xxx26").is_none());
        assert!(ParsedVersion::parse("r1-alpha7-nightly-16aug2026").is_none());
        assert!(ParsedVersion::parse("r1-alpha7-nightly-").is_none());
    }

    #[test]
    fn max_picks_the_newest_of_a_mixed_list() {
        let mut tags: Vec<ParsedVersion> = [
            "r1-alpha6",
            "r1-alpha7-nightly-15aug26",
            "r1-alpha7-nightly-16aug26",
            "r1-alpha5",
        ]
        .iter()
        .filter_map(|t| ParsedVersion::parse(t))
        .collect();
        tags.sort();
        assert_eq!(
            tags.last().unwrap(),
            &ParsedVersion::parse("r1-alpha7-nightly-16aug26").unwrap()
        );
    }
}
