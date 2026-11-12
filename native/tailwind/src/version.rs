//! The version of Tailwind CSS that the compiler follows now.
//!
//! The Rust compiler is a port of one version of Tailwind CSS
//! (`NATIVE`). It can also compile as an older version: each behavior that
//! changed in a release is a switch (`switches.rs`), and a switch is on
//! when the selected version has the release of that change.
//!
//! The version is a value of the thread, for the time of one `compile` or
//! one build (`enter`). So a switch needs no extra parameter: the code
//! asks `switches::NAME.on()` at the place of the change.

use std::cell::Cell;

/// A version of Tailwind CSS: major, minor, patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub u16, pub u16, pub u16);

impl Version {
    /// The version of the port: the newest behavior, with each switch on.
    pub const NATIVE: Version = Version(4, 3, 5);

    /// The oldest version that the Rust compiler can compile as: for each
    /// later change of Tailwind CSS it has a switch, and the fuzzer finds
    /// no difference from the official compiler of that version
    /// (docs/VERSIONS.md). An older version uses the script engine.
    pub const OLDEST: Version = Version(4, 0, 8);

    /// The oldest version whose plugin layer the program has. The plugin
    /// layer (`compat/` of Tailwind CSS) is JavaScript: the program has
    /// the file of each version from this one to `NATIVE`, made from the
    /// source of that version with no change (`scripts/compat.sh`,
    /// `compat/js/bundles.rs`). For an older version, the Rust compiler
    /// does not compile a stylesheet with `@plugin` or `@config`: it
    /// gives the error `SCRIPT_ENGINE_NEEDED`, and the caller uses the
    /// script engine. When this is `OLDEST`, that does not occur.
    pub const OLDEST_WITH_PLUGINS: Version = Version(4, 0, 8);

    /// `4.3.5` as a version. A text with another form is `None`.
    pub fn parse(text: &str) -> Option<Version> {
        let mut parts = text.split('.');
        let version = Version(parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
        parts.next().is_none().then_some(version)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// The name of the error of the Rust compiler for a stylesheet that it
/// does not compile as the selected version (see
/// `Version::OLDEST_WITH_PLUGINS`).
pub const SCRIPT_ENGINE_NEEDED: &str = "ScriptEngineNeeded";

thread_local! {
    static CURRENT: Cell<Version> = const { Cell::new(Version::NATIVE) };
}

/// The version that the compiler follows now.
pub fn current() -> Version {
    CURRENT.with(Cell::get)
}

/// Puts the version of before back when it goes out of scope.
pub struct Guard(Version);

impl Drop for Guard {
    fn drop(&mut self) {
        CURRENT.with(|current| current.set(self.0));
    }
}

/// Follow this version until the guard goes out of scope.
#[must_use]
pub fn enter(version: Version) -> Guard {
    Guard(CURRENT.with(|current| current.replace(version)))
}

/// A behavior that changed in a release of Tailwind CSS.
#[derive(Debug)]
pub struct Switch {
    /// The first version with the new behavior.
    pub since: Version,
    /// What changed, in one sentence.
    pub what: &'static str,
}

impl Switch {
    pub const fn since(major: u16, minor: u16, patch: u16, what: &'static str) -> Self {
        Self { since: Version(major, minor, patch), what }
    }

    /// True when the selected version has the new behavior.
    pub fn on(&self) -> bool {
        current() >= self.since
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_native_version_is_the_version_of_the_port() {
        assert_eq!(Version::NATIVE.to_string(), crate::index::VERSION);
        assert_eq!(Version::parse(crate::index::VERSION), Some(Version::NATIVE));
        assert!(Version::OLDEST <= Version::NATIVE);
    }

    #[test]
    fn parses_and_compares_versions() {
        assert_eq!(Version::parse("4.10.2"), Some(Version(4, 10, 2)));
        assert_eq!(Version::parse("4.3"), None);
        assert_eq!(Version::parse("4.3.5.1"), None);
        assert_eq!(Version::parse("4.3.x"), None);
        assert!(Version(4, 10, 0) > Version(4, 9, 9));
    }

    #[test]
    fn a_switch_follows_the_version_of_the_thread() {
        const NEW: Switch = Switch::since(4, 3, 4, "a test");
        assert!(NEW.on());
        {
            let _guard = enter(Version(4, 3, 3));
            assert!(!NEW.on());
            {
                let _inner = enter(Version(4, 3, 4));
                assert!(NEW.on());
            }
            assert!(!NEW.on());
        }
        assert!(NEW.on());
    }
}
