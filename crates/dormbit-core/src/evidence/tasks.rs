//! Kiro task counting (design: How Dockline's "3 of 7 tasks complete" is derived).

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskCount {
    pub done: u32,
    pub total: u32,
    pub sub_done: u32,
    pub sub_total: u32,
    pub first_line: u32,
    pub last_line: u32,
    /// (line, title) of each unchecked top-level task.
    pub unchecked: Vec<(u32, String)>,
}

fn fence_marker(line: &str) -> Option<(char, usize)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let c = rest.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let n = rest.chars().take_while(|&x| x == c).count();
    (n >= 3).then_some((c, n))
}

/// `- [m]` (optionally `- [m]*`) followed by a space or the end of the line.
fn checkbox(s: &str) -> Option<(char, &str)> {
    let rest = s.strip_prefix("- [")?;
    let mut it = rest.chars();
    let mark = it.next()?;
    let after = it.as_str().strip_prefix(']')?;
    let after = after.strip_prefix('*').unwrap_or(after);
    if after.is_empty() || after.starts_with(' ') {
        Some((mark, after.trim()))
    } else {
        None
    }
}

pub fn count_tasks(text: &str) -> TaskCount {
    let mut c = TaskCount::default();
    let mut fence: Option<(char, usize)> = None;
    for (i, line) in text.lines().enumerate() {
        let n = u32::try_from(i + 1).unwrap_or(u32::MAX);
        if let Some((ch, len)) = fence {
            if let Some((ch2, len2)) = fence_marker(line)
                && ch2 == ch
                && len2 >= len
                && line.trim().chars().all(|x| x == ch)
            {
                fence = None;
            }
            continue;
        }
        if let Some(f) = fence_marker(line) {
            fence = Some(f);
            continue;
        }
        if let Some((mark, title)) = checkbox(line) {
            let done = mark == 'x' || mark == 'X';
            c.total += 1;
            if done {
                c.done += 1;
            } else {
                c.unchecked.push((n, title.to_owned()));
            }
            if c.first_line == 0 {
                c.first_line = n;
            }
            c.last_line = n;
        } else if line.starts_with([' ', '\t'])
            && let Some((mark, _)) = checkbox(line.trim_start_matches([' ', '\t']))
        {
            c.sub_total += 1;
            if mark == 'x' || mark == 'X' {
                c.sub_done += 1;
            }
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_top_level_tasks_and_ignores_fences() {
        let t = "# Plan\n\n- [x] 1. A\n- [ ] 2. B\n  - [x] 2.1 sub\n```\n- [x] not a task\n```\n- [X] 3. C\n- [ ]* 4. optional\n- [~] 5. other mark\n-[x] no space\n";
        let c = count_tasks(t);
        assert_eq!((c.done, c.total), (2, 5));
        assert_eq!((c.sub_done, c.sub_total), (1, 1));
        assert_eq!(c.first_line, 3);
        assert_eq!(
            c.unchecked.iter().map(|u| u.0).collect::<Vec<_>>(),
            vec![4, 10, 11]
        );
        assert_eq!(c.unchecked[0].1, "2. B");
    }
}
