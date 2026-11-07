use std::fmt;

use crate::model::{DormbitError, ErrorCode};

const MAX_COMPONENT: usize = 255;
const MAX_PATH: usize = 1024;

/// A validated project-relative path (capsule-format §6.1): `/`-separated, no empty, `.` or
/// `..` components, no NUL, control, or backslash characters, components at most 255 bytes and
/// the whole path at most 1024. The empty path is the project root and is made only by
/// [`RelPath::root`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPath(String);

impl RelPath {
    /// The project root.
    pub fn root() -> Self {
        Self(String::new())
    }

    /// Parses a non-empty relative path, or returns `PATH_UNSAFE`.
    pub fn parse(s: &str) -> Result<Self, DormbitError> {
        if s.is_empty() || s.len() > MAX_PATH || s.starts_with('/') {
            return Err(unsafe_path(s));
        }
        for comp in s.split('/') {
            check_component(comp).map_err(|()| unsafe_path(s))?;
        }
        Ok(Self(s.to_owned()))
    }

    /// Appends one validated component.
    pub fn join(&self, name: &str) -> Result<Self, DormbitError> {
        check_component(name).map_err(|()| unsafe_path(name))?;
        let joined = if self.0.is_empty() {
            name.to_owned()
        } else {
            format!("{}/{name}", self.0)
        };
        if joined.len() > MAX_PATH {
            return Err(unsafe_path(&joined));
        }
        Ok(Self(joined))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// The last component, or `""` for the root.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or("")
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn check_component(c: &str) -> Result<(), ()> {
    if c.is_empty() || c == "." || c == ".." || c.len() > MAX_COMPONENT || c.contains('/') {
        return Err(());
    }
    // C0 (including NUL), DEL, C1 controls, and the backslash are all refused.
    if c.chars()
        .any(|ch| ch == '\\' || ch == '\u{7f}' || ch.is_control())
    {
        return Err(());
    }
    Ok(())
}

fn unsafe_path(s: &str) -> DormbitError {
    // Never echo control characters back into a message.
    let shown: String = s
        .chars()
        .map(|c| if c.is_control() { '\u{fffd}' } else { c })
        .take(120)
        .collect();
    DormbitError::new(
        ErrorCode::PathUnsafe,
        format!("\"{shown}\" is not a safe project path."),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_paths() {
        for p in [
            "a",
            "src/lib.rs",
            ".git/config",
            "dir with space/é.txt",
            "日本語/ファイル",
        ] {
            assert!(RelPath::parse(p).is_ok(), "{p}");
        }
    }

    #[test]
    fn refuses_every_unsafe_shape() {
        let long_component = "a".repeat(256);
        let long_path = vec!["abcdefghij"; 120].join("/");
        for p in [
            "",
            "/abs",
            "a//b",
            "a/",
            "./a",
            "a/./b",
            "../a",
            "a/../b",
            "a\\b",
            "nul\0byte",
            "tab\tchar",
            "del\u{7f}",
            "c1\u{85}",
            long_component.as_str(),
            long_path.as_str(),
        ] {
            let err = RelPath::parse(p).expect_err(p);
            assert_eq!(err.code, ErrorCode::PathUnsafe, "{p:?}");
        }
    }

    #[test]
    fn join_validates_and_the_root_joins_cleanly() {
        let root = RelPath::root();
        assert!(root.is_root());
        let a = root.join("a").unwrap();
        assert_eq!(a.join("b").unwrap().as_str(), "a/b");
        assert!(a.join("..").is_err());
        assert!(
            a.join("x/y").is_err(),
            "a slash inside a component is refused"
        );
    }

    #[test]
    fn error_messages_do_not_echo_control_characters() {
        let err = RelPath::parse("bad\u{1b}[31m").unwrap_err();
        assert!(
            !err.message.chars().any(|c| c.is_control()),
            "{}",
            err.message
        );
    }

    #[test]
    fn file_name_is_the_last_component() {
        assert_eq!(RelPath::parse("a/b/c.txt").unwrap().file_name(), "c.txt");
        assert_eq!(RelPath::root().file_name(), "");
    }
}
