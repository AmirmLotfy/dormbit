//! A small secret-like-content scanner (CAP-3). It reports the first matching line number and
//! nothing else: the matched text is never returned, logged, or put in an error.

/// Where the first secret-like text was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretHit {
    /// 1-based line number.
    pub line: u32,
}

/// The outcome of scanning one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scan {
    Clean,
    Hit(SecretHit),
    /// Binary content (a NUL byte in the first 8 KiB), not scanned.
    SkippedBinary,
}

const BINARY_PROBE: usize = 8192;

pub fn scan_secrets(bytes: &[u8]) -> Scan {
    if bytes.iter().take(BINARY_PROBE).any(|&b| b == 0) {
        return Scan::SkippedBinary;
    }
    let text = String::from_utf8_lossy(bytes);
    for (i, line) in text.lines().enumerate() {
        if line_has_secret(line) {
            return Scan::Hit(SecretHit {
                line: u32::try_from(i + 1).unwrap_or(u32::MAX),
            });
        }
    }
    Scan::Clean
}

fn line_has_secret(line: &str) -> bool {
    has_token_prefix(line)
        || has_private_key_header(line)
        || has_url_password(line)
        || has_secret_assignment(line)
}

fn run_len(s: &str, allow: impl Fn(char) -> bool) -> usize {
    s.chars().take_while(|&c| allow(c)).count()
}

fn has_token_prefix(line: &str) -> bool {
    // (prefix, minimum run of token characters after it, uppercase-and-digits only)
    const RULES: [(&str, usize, bool); 11] = [
        ("AKIA", 16, true),
        ("ASIA", 16, true),
        ("ghp_", 36, false),
        ("gho_", 36, false),
        ("ghu_", 36, false),
        ("ghs_", 36, false),
        ("ghr_", 36, false),
        ("github_pat_", 22, false),
        ("sk_live_", 24, false),
        ("xoxb-", 10, false),
        ("xoxp-", 10, false),
    ];
    for (prefix, min, upper_only) in RULES {
        let mut from = 0;
        while let Some(pos) = line[from..].find(prefix) {
            let start = from + pos;
            let boundary = line[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_ascii_alphanumeric());
            let rest = &line[start + prefix.len()..];
            let n = run_len(rest, |c| {
                if upper_only {
                    c.is_ascii_uppercase() || c.is_ascii_digit()
                } else {
                    c.is_ascii_alphanumeric() || c == '_' || c == '-'
                }
            });
            if boundary && n >= min {
                return true;
            }
            from = start + prefix.len();
        }
    }
    false
}

fn has_private_key_header(line: &str) -> bool {
    line.contains("-----BEGIN") && line.contains("PRIVATE KEY-----")
}

fn is_placeholder(value: &str) -> bool {
    let v = value.trim();
    let lower = v.to_ascii_lowercase();
    v.len() < 6
        || v.starts_with(['$', '<', '{', '%'])
        || v.chars().all(|c| matches!(c, '*' | 'x' | 'X' | '.'))
        || [
            "changeme",
            "change-me",
            "example",
            "placeholder",
            "your_",
            "your-",
            "redacted",
            "undefined",
            "process.env",
        ]
        .iter()
        .any(|p| lower.contains(p))
}

fn has_url_password(line: &str) -> bool {
    let mut from = 0;
    while let Some(pos) = line[from..].find("://") {
        let after = &line[from + pos + 3..];
        let authority = after
            .split(['/', ' ', '\t', '"', '\'', '`', ')', '>'])
            .next()
            .unwrap_or("");
        if let Some((userinfo, _host)) = authority.rsplit_once('@')
            && let Some((user, pass)) = userinfo.split_once(':')
            && !user.is_empty()
            && !is_placeholder(pass)
        {
            return true;
        }
        from += pos + 3;
    }
    false
}

fn has_secret_assignment(line: &str) -> bool {
    const KEYS: [&str; 6] = ["password", "passwd", "secret", "token", "api_key", "apikey"];
    let lower = line.to_ascii_lowercase();
    let trimmed = lower.trim_start();
    for key in KEYS {
        let mut from = 0;
        while let Some(pos) = lower[from..].find(key) {
            let start = from + pos;
            // Identifier characters may follow the key (`_KEY`, `Hash`) before the separator.
            let tail = run_len(&line[start + key.len()..], |c| {
                c.is_ascii_alphanumeric() || c == '_'
            });
            let rest = line[start + key.len() + tail..].trim_start_matches(['"', '\'', ' ', '\t']);
            let Some(after_sep) = rest.strip_prefix('=').or_else(|| rest.strip_prefix(':')) else {
                from = start + key.len();
                continue;
            };
            let value = after_sep.trim_start();
            if let Some(q) = value.chars().next().filter(|c| *c == '"' || *c == '\'') {
                let body = &value[1..];
                let literal = &body[..body.find(q).unwrap_or(body.len())];
                if !is_placeholder(literal) && !literal.contains(' ') {
                    return true;
                }
            } else if trimmed.starts_with(key)
                || line
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_uppercase())
            {
                // Env style (`NAME_TOKEN=value`) with a bare value.
                let bare: String = value
                    .chars()
                    .take_while(|c| !c.is_whitespace() && *c != '#')
                    .collect();
                if bare.len() >= 8 && !is_placeholder(&bare) {
                    return true;
                }
            }
            from = start + key.len();
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test inputs are assembled at run time from short repeated fragments, so no secret-like
    /// literal is ever committed to the repository (CAP-3, Requirement 3.7).
    fn word(a: &str, b: &str) -> String {
        format!("{a}{b}")
    }

    fn body(unit: &str, n: usize) -> String {
        unit.repeat(n)
    }

    fn hit_line(text: &str) -> Option<u32> {
        match scan_secrets(text.as_bytes()) {
            Scan::Hit(h) => Some(h.line),
            _ => None,
        }
    }

    #[test]
    fn finds_known_token_prefixes_and_reports_only_the_line() {
        let aws = word("AK", "IA") + &body("Q7", 8);
        let gh = word("gh", "p_") + &body("aB3d", 9);
        let live = word("sk", "_live_") + &body("Z9", 14);
        let slack = word("xo", "xb-") + &body("12", 6);
        assert_eq!(hit_line(&format!("id = {aws}")), Some(1));
        assert_eq!(hit_line(&format!("ok\n\nuse {gh} here")), Some(3));
        assert_eq!(hit_line(&format!("a\nb\nc\n{live}")), Some(4));
        assert_eq!(hit_line(&slack), Some(1));
    }

    #[test]
    fn finds_private_key_headers_url_passwords_and_assignments() {
        let pem = word("-----BEGIN RSA PRIV", "ATE KEY-----");
        let url = word("postgres://app:", &body("pw", 6)) + "@db.internal:5432/app";
        let quoted = format!("const {} = \"{}\";", word("pass", "word"), body("zk", 8));
        let env = format!("API_{}={}", "TOKEN", body("k9", 6));
        for text in [pem, url, quoted, env] {
            assert_eq!(hit_line(&text), Some(1), "{}", &text[..6]);
        }
    }

    #[test]
    fn ignores_placeholders_empty_values_and_plain_code() {
        for text in [
            "password: string;",
            "const token = process.env.TOKEN;",
            "SECRET=",
            "API_TOKEN=${API_TOKEN}",
            "password = \"\"",
            "password = \"changeme\"",
            "url = \"https://user@example.com/path\"",
            "tokenize(input) and secretary are words",
            "AKIA is a prefix",
        ] {
            assert_eq!(scan_secrets(text.as_bytes()), Scan::Clean, "{text}");
        }
    }

    #[test]
    fn binary_files_are_skipped_not_scanned() {
        let mut b = vec![0u8, 1, 2];
        b.extend_from_slice((word("AK", "IA") + &body("Q7", 8)).as_bytes());
        assert_eq!(scan_secrets(&b), Scan::SkippedBinary);
    }

    #[test]
    fn a_hit_carries_only_a_line_number() {
        assert_eq!(
            format!("{:?}", SecretHit { line: 7 }),
            "SecretHit { line: 7 }"
        );
    }
}
