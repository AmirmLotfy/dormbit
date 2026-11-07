//! Restore path rules and comparison keys (design: Keys and path rules).
//!
//! `SafeName` and `SafePath` have private fields and are built only here, so the executor
//! can never receive an unchecked name.

use unicode_normalization::UnicodeNormalization;

pub const STAGING_PREFIX: &str = ".dormbit-staging-";
const MAX_COMPONENT: usize = 255;
const MAX_PATH: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRule {
    Absolute,
    DotOrDotDot,
    EmptyComponent,
    ControlChar,
    ComponentTooLong,
    PathTooLong,
    Reserved,
}

impl PathRule {
    pub fn describe(self) -> &'static str {
        match self {
            PathRule::Absolute => "is an absolute path",
            PathRule::DotOrDotDot => "contains `.` or `..`",
            PathRule::EmptyComponent => "has an empty part",
            PathRule::ControlChar => "contains a control character",
            PathRule::ComponentTooLong => "has a name longer than 255 bytes",
            PathRule::PathTooLong => "is longer than 1024 bytes",
            PathRule::Reserved => "uses a name DORMBIT reserves",
        }
    }
}

/// One validated path component.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SafeName(String);

impl SafeName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated relative path of one or more components.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SafePath(Vec<SafeName>);

impl SafePath {
    pub fn components(&self) -> &[SafeName] {
        &self.0
    }

    pub fn parent(&self) -> &[SafeName] {
        &self.0[..self.0.len().saturating_sub(1)]
    }

    pub fn file_name(&self) -> Option<&SafeName> {
        self.0.last()
    }

    pub fn display(&self) -> String {
        self.0
            .iter()
            .map(SafeName::as_str)
            .collect::<Vec<_>>()
            .join("/")
    }
}

fn is_cc(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}')
}

fn component(c: &str) -> Result<SafeName, PathRule> {
    if c.is_empty() {
        return Err(PathRule::EmptyComponent);
    }
    if c == "." || c == ".." {
        return Err(PathRule::DotOrDotDot);
    }
    if c.chars().any(is_cc) {
        return Err(PathRule::ControlChar);
    }
    if c.len() > MAX_COMPONENT {
        return Err(PathRule::ComponentTooLong);
    }
    Ok(SafeName(c.to_owned()))
}

/// A manifest entry path (Requirements 5.1-5.5).
pub fn validate_entry_path(raw: &str) -> Result<SafePath, PathRule> {
    if raw.starts_with('/') {
        return Err(PathRule::Absolute);
    }
    if raw.len() > MAX_PATH {
        return Err(PathRule::PathTooLong);
    }
    let parts = raw
        .split('/')
        .map(component)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SafePath(parts))
}

/// The name of the new folder to restore into (Requirement 1.6).
pub fn validate_target_name(raw: &str) -> Result<SafeName, PathRule> {
    if raw.contains('/') {
        return Err(if raw.starts_with('/') {
            PathRule::Absolute
        } else {
            PathRule::EmptyComponent
        });
    }
    let n = component(raw)?;
    if raw.starts_with(STAGING_PREFIX) || raw.starts_with(".dormbit-capture-") {
        return Err(PathRule::Reserved);
    }
    Ok(n)
}

/// A restore op ID: 1-64 characters of hex digits and hyphens (a UUID in practice).
pub fn is_op_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

/// `.dormbit-staging-<op_id>` and nothing else.
pub fn is_staging_name(s: &str) -> bool {
    s.strip_prefix(STAGING_PREFIX).is_some_and(is_op_id)
}

/// A name DORMBIT itself creates (staging folder, owner file). Not user input.
pub(crate) fn internal_name(raw: &str) -> SafeName {
    SafeName(raw.to_owned())
}

pub fn nfc_key(c: &str) -> String {
    c.nfc().collect()
}

/// Case-fold key: NFC(lower(upper(NFD(c)))), which matches the default APFS comparison
/// closely enough to catch every pair a case-insensitive volume treats as one name.
pub fn fold_key(c: &str) -> String {
    let nfd: String = c.nfd().collect();
    nfd.to_uppercase().to_lowercase().nfc().collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMode {
    Nfc,
    NfcAndFold,
}

/// The comparison key of a whole path under `mode`.
pub fn path_key(p: &SafePath, mode: KeyMode) -> String {
    p.components()
        .iter()
        .map(|c| match mode {
            KeyMode::Nfc => nfc_key(c.as_str()),
            KeyMode::NfcAndFold => fold_key(c.as_str()),
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_paths_follow_every_rule() {
        assert!(validate_entry_path("src/main.rs").is_ok());
        assert_eq!(validate_entry_path("/etc/x"), Err(PathRule::Absolute));
        assert_eq!(validate_entry_path("a/../b"), Err(PathRule::DotOrDotDot));
        assert_eq!(validate_entry_path("./a"), Err(PathRule::DotOrDotDot));
        assert_eq!(validate_entry_path("a//b"), Err(PathRule::EmptyComponent));
        assert_eq!(validate_entry_path(""), Err(PathRule::EmptyComponent));
        assert_eq!(validate_entry_path("a\u{1}b"), Err(PathRule::ControlChar));
        assert_eq!(validate_entry_path("a\u{85}b"), Err(PathRule::ControlChar));
        assert!(validate_entry_path(&"x".repeat(255)).is_ok());
        assert_eq!(
            validate_entry_path(&"x".repeat(256)),
            Err(PathRule::ComponentTooLong)
        );
        let long = vec!["y".repeat(200); 6].join("/");
        assert_eq!(validate_entry_path(&long), Err(PathRule::PathTooLong));
    }

    #[test]
    fn target_names_are_one_component_and_not_reserved() {
        assert!(validate_target_name("dockline-restored").is_ok());
        assert!(validate_target_name("a/b").is_err());
        assert!(validate_target_name("..").is_err());
        assert_eq!(
            validate_target_name(".dormbit-staging-x"),
            Err(PathRule::Reserved)
        );
    }

    #[test]
    fn keys_catch_case_and_normalization_variants() {
        let nfd = "Cafe\u{301}";
        let nfc = "Caf\u{e9}";
        assert_eq!(nfc_key(nfd), nfc_key(nfc));
        assert_eq!(fold_key("README.md"), fold_key("readme.MD"));
        assert_eq!(fold_key(nfd), fold_key("CAFÉ"));
        assert_ne!(nfc_key("README.md"), nfc_key("readme.md"));
        assert_eq!(fold_key("Straße"), fold_key("STRASSE"));
    }
}
