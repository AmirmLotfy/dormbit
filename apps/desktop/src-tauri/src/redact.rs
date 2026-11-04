//! Home-directory redaction for text that crosses the IPC boundary.

use std::path::Path;

/// Replaces a leading or embedded `home` path with `~`, matching on path boundaries only.
///
/// `/Users/ann/x` with home `/Users/ann` becomes `~/x`; `/Users/annette/x` is left alone.
/// An empty or root home is ignored so the whole string can't be rewritten by accident.
pub fn redact_home(s: &str, home: &Path) -> String {
    let Some(home) = home.to_str() else {
        return s.to_owned();
    };
    let home = home.trim_end_matches('/');
    if home.is_empty() {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find(home) {
        let after = &rest[pos + home.len()..];
        let boundary = after.is_empty()
            || !after
                .starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '-' || c == '.');
        out.push_str(&rest[..pos]);
        if boundary {
            out.push('~');
        } else {
            out.push_str(home);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_home_prefix() {
        let h = Path::new("/Users/ann");
        assert_eq!(redact_home("/Users/ann/dev/x", h), "~/dev/x");
        assert_eq!(redact_home("cannot read /Users/ann", h), "cannot read ~");
    }

    #[test]
    fn leaves_a_longer_sibling_name_alone() {
        let h = Path::new("/Users/ann");
        assert_eq!(redact_home("/Users/annette/x", h), "/Users/annette/x");
    }

    #[test]
    fn replaces_every_occurrence() {
        let h = Path::new("/Users/ann");
        assert_eq!(redact_home("/Users/ann/a -> /Users/ann/b", h), "~/a -> ~/b");
    }

    #[test]
    fn ignores_an_empty_or_root_home() {
        assert_eq!(redact_home("/etc/hosts", Path::new("")), "/etc/hosts");
        assert_eq!(redact_home("/etc/hosts", Path::new("/")), "/etc/hosts");
    }

    #[test]
    fn handles_a_trailing_slash_on_home() {
        assert_eq!(redact_home("/Users/ann/x", Path::new("/Users/ann/")), "~/x");
    }
}
