//! A small `.gitignore` matcher for the root `.gitignore` of a project (v1 subset).
//!
//! Supported: comments, blank lines, `!` negation, a trailing `/` (directories only), a leading
//! `/` or an inner `/` (anchored to the root), `*`, `?`, and `**` path segments. Not supported:
//! nested `.gitignore` files, `.git/info/exclude`, the global ignore file, and `[abc]` classes
//! (a `[` matches itself). The plan labels gitignored files "needs decision", so a miss here
//! costs the user one extra question, never a lost file.

#[derive(Debug, Clone)]
struct Rule {
    segments: Vec<String>,
    negated: bool,
    dir_only: bool,
    anchored: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Gitignore {
    rules: Vec<Rule>,
}

impl Gitignore {
    pub fn parse(text: &str) -> Self {
        let mut rules = Vec::new();
        for raw in text.lines() {
            let line = raw.trim_end_matches(['\r', ' ']);
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (negated, line) = match line.strip_prefix('!') {
                Some(rest) => (true, rest),
                None => (false, line),
            };
            let (dir_only, line) = match line.strip_suffix('/') {
                Some(rest) => (true, rest),
                None => (false, line),
            };
            let anchored = line.contains('/');
            let line = line.strip_prefix('/').unwrap_or(line);
            if line.is_empty() {
                continue;
            }
            rules.push(Rule {
                segments: line.split('/').map(str::to_owned).collect(),
                negated,
                dir_only,
                anchored,
            });
        }
        Self { rules }
    }

    /// Whether `path` (project-relative) is ignored. Ancestor directories count: a file inside
    /// an ignored folder is ignored. The last matching rule wins.
    pub fn is_ignored(&self, path: &str, is_dir: bool) -> bool {
        let comps: Vec<&str> = path.split('/').collect();
        let mut ignored = false;
        for end in 1..=comps.len() {
            let candidate = &comps[..end];
            let candidate_is_dir = end < comps.len() || is_dir;
            for rule in &self.rules {
                if rule.dir_only && !candidate_is_dir {
                    continue;
                }
                let hit = if rule.anchored {
                    match_segments(&rule.segments, candidate)
                } else {
                    candidate
                        .last()
                        .is_some_and(|last| match_glob(&rule.segments[0], last))
                };
                if hit {
                    ignored = !rule.negated;
                }
            }
        }
        ignored
    }
}

fn match_segments(pattern: &[String], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((first, rest)) if first == "**" => {
            // `**` matches zero or more whole segments.
            (0..=path.len()).any(|skip| match_segments(rest, &path[skip..]))
        }
        Some((first, rest)) => match path.split_first() {
            Some((head, tail)) => match_glob(first, head) && match_segments(rest, tail),
            None => false,
        },
    }
}

/// `*` (any run, not crossing `/`, which never appears in a segment) and `?` (one character).
fn match_glob(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) && p[pi] != '*' {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gi(text: &str) -> Gitignore {
        Gitignore::parse(text)
    }

    #[test]
    fn matches_names_at_any_depth_and_whole_folders() {
        let g = gi("node_modules/\n.env\n*.log\n");
        assert!(g.is_ignored("node_modules", true));
        assert!(g.is_ignored("apps/web/node_modules/pkg/index.js", false));
        assert!(g.is_ignored(".env", false));
        assert!(g.is_ignored("a/b/debug.log", false));
        assert!(!g.is_ignored("src/main.rs", false));
        assert!(
            !g.is_ignored("node_modules", false),
            "a dir-only rule doesn't match a file of that name"
        );
    }

    #[test]
    fn anchored_patterns_match_from_the_root_only() {
        let g = gi("/build\ndata/*.sqlite\ndocs/**/draft.md\n");
        assert!(g.is_ignored("build", true));
        assert!(!g.is_ignored("src/build", true));
        assert!(g.is_ignored("data/dev.sqlite", false));
        assert!(!g.is_ignored("data/sub/dev.sqlite", false));
        assert!(g.is_ignored("docs/draft.md", false));
        assert!(g.is_ignored("docs/a/b/draft.md", false));
    }

    #[test]
    fn negation_and_last_match_win() {
        let g = gi("*.log\n!keep.log\n");
        assert!(g.is_ignored("a.log", false));
        assert!(!g.is_ignored("keep.log", false));
        let g = gi("!keep.log\n*.log\n");
        assert!(g.is_ignored("keep.log", false));
    }

    #[test]
    fn question_mark_and_star_stay_inside_a_segment() {
        let g = gi("a?c\nx*z\n");
        assert!(g.is_ignored("abc", false));
        assert!(!g.is_ignored("abbc", false));
        assert!(g.is_ignored("xyyz", false));
        assert!(g.is_ignored("xz", false));
    }

    #[test]
    fn comments_blank_lines_and_crlf_are_ignored() {
        let g = gi("# comment\r\n\r\n   \ndist/\r\n");
        assert!(g.is_ignored("dist", true));
        assert!(!g.is_ignored("# comment", false));
    }
}
