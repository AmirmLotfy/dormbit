use crate::model::Decision;

/// Why an item has the decision it has. Each code maps to exactly one [`Group`], one
/// editability, and one default decision, so the plan never stores them separately and they
/// can't drift (design: classification precedence).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReasonCode {
    UnsafeName,
    SpecialFile,
    SensitiveEnv,
    SensitiveCert,
    SensitivePrivateKey,
    SensitiveCredentials,
    GitHooks,
    GitLock,
    GitFsmonitor,
    NameCollision,
    Rebuildable,
    Unreadable,
    OverSizeLimit,
    SymlinkRecordOnly,
    DataFile,
    GitignoredNotRebuildable,
    GitInternal,
    GitWorkingTreeOnly,
    ProjectFile,
}

/// The plan screen's groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Included,
    NeedsDecision,
    ExcludedSensitive,
    ExcludedRebuildable,
    ExcludedGit,
    Unsupported,
    Unreadable,
    /// Editable items the user chose to leave out.
    ExcludedByYou,
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Group::Included => "Included",
            Group::NeedsDecision => "Needs decision",
            Group::ExcludedSensitive => "Excluded: sensitive",
            Group::ExcludedRebuildable => "Excluded: rebuildable",
            Group::ExcludedGit => "Excluded: Git",
            Group::Unsupported => "Unsupported",
            Group::Unreadable => "Unreadable",
            Group::ExcludedByYou => "Excluded: by you",
        }
    }

    /// The stable code written into the manifest's `excluded[].group`.
    pub fn code(self) -> &'static str {
        match self {
            Group::Included => "included",
            Group::NeedsDecision => "needs_decision",
            Group::ExcludedSensitive => "excluded_sensitive",
            Group::ExcludedRebuildable => "excluded_rebuildable",
            Group::ExcludedGit => "excluded_git",
            Group::Unsupported => "unsupported",
            Group::Unreadable => "unreadable",
            Group::ExcludedByYou => "excluded_by_you",
        }
    }

    pub const ALL: [Group; 8] = [
        Group::Included,
        Group::NeedsDecision,
        Group::ExcludedSensitive,
        Group::ExcludedRebuildable,
        Group::ExcludedGit,
        Group::Unsupported,
        Group::Unreadable,
        Group::ExcludedByYou,
    ];
}

const TABLE: [(ReasonCode, &str); 19] = [
    (ReasonCode::UnsafeName, "unsafe_name"),
    (ReasonCode::SpecialFile, "special_file"),
    (ReasonCode::SensitiveEnv, "sensitive_env"),
    (ReasonCode::SensitiveCert, "sensitive_cert"),
    (ReasonCode::SensitivePrivateKey, "sensitive_private_key"),
    (ReasonCode::SensitiveCredentials, "sensitive_credentials"),
    (ReasonCode::GitHooks, "git_hooks"),
    (ReasonCode::GitLock, "git_lock"),
    (ReasonCode::GitFsmonitor, "git_fsmonitor"),
    (ReasonCode::NameCollision, "name_collision"),
    (ReasonCode::Rebuildable, "rebuildable"),
    (ReasonCode::Unreadable, "unreadable"),
    (ReasonCode::OverSizeLimit, "over_size_limit"),
    (ReasonCode::SymlinkRecordOnly, "symlink_record_only"),
    (ReasonCode::DataFile, "data_file"),
    (
        ReasonCode::GitignoredNotRebuildable,
        "gitignored_not_rebuildable",
    ),
    (ReasonCode::GitInternal, "git_internal"),
    (ReasonCode::GitWorkingTreeOnly, "git_working_tree_only"),
    (ReasonCode::ProjectFile, "project_file"),
];

impl ReasonCode {
    pub const ALL: [ReasonCode; 19] = [
        ReasonCode::UnsafeName,
        ReasonCode::SpecialFile,
        ReasonCode::SensitiveEnv,
        ReasonCode::SensitiveCert,
        ReasonCode::SensitivePrivateKey,
        ReasonCode::SensitiveCredentials,
        ReasonCode::GitHooks,
        ReasonCode::GitLock,
        ReasonCode::GitFsmonitor,
        ReasonCode::NameCollision,
        ReasonCode::Rebuildable,
        ReasonCode::Unreadable,
        ReasonCode::OverSizeLimit,
        ReasonCode::SymlinkRecordOnly,
        ReasonCode::DataFile,
        ReasonCode::GitignoredNotRebuildable,
        ReasonCode::GitInternal,
        ReasonCode::GitWorkingTreeOnly,
        ReasonCode::ProjectFile,
    ];

    pub fn as_str(self) -> &'static str {
        TABLE
            .iter()
            .find(|(c, _)| *c == self)
            .map_or("project_file", |(_, s)| s)
    }

    pub fn parse(s: &str) -> Option<ReasonCode> {
        TABLE.iter().find(|(_, name)| *name == s).map(|(c, _)| *c)
    }

    pub fn group(self) -> Group {
        use ReasonCode::*;
        match self {
            UnsafeName | SpecialFile | GitHooks | GitLock | GitFsmonitor | NameCollision
            | OverSizeLimit => Group::Unsupported,
            SensitiveEnv | SensitiveCert | SensitivePrivateKey | SensitiveCredentials => {
                Group::ExcludedSensitive
            }
            Rebuildable => Group::ExcludedRebuildable,
            Unreadable => Group::Unreadable,
            DataFile | GitignoredNotRebuildable => Group::NeedsDecision,
            GitWorkingTreeOnly => Group::ExcludedGit,
            SymlinkRecordOnly | GitInternal | ProjectFile => Group::Included,
        }
    }

    /// Whether the user may change the decision. Sensitive, unsupported, unreadable, symlink,
    /// and Git-internal items are locked (CAP-3, CAP-4).
    pub fn editable(self) -> bool {
        matches!(
            self,
            ReasonCode::Rebuildable
                | ReasonCode::DataFile
                | ReasonCode::GitignoredNotRebuildable
                | ReasonCode::ProjectFile
        )
    }

    /// The decision the rule gives before the user touches anything.
    pub fn default_decision(self) -> Decision {
        match self.group() {
            Group::Included => Decision::Include,
            Group::NeedsDecision => Decision::Undecided,
            _ => Decision::Exclude,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips_through_its_name() {
        for c in ReasonCode::ALL {
            assert_eq!(ReasonCode::parse(c.as_str()), Some(c), "{c:?}");
        }
        assert_eq!(ReasonCode::parse("nope"), None);
        assert_eq!(TABLE.len(), ReasonCode::ALL.len());
    }

    #[test]
    fn locked_codes_never_default_to_include() {
        for c in ReasonCode::ALL {
            if !c.editable() && c.group() != Group::Included {
                assert_eq!(c.default_decision(), Decision::Exclude, "{c:?}");
            }
        }
    }

    #[test]
    fn sensitive_codes_are_locked_and_excluded() {
        for c in [
            ReasonCode::SensitiveEnv,
            ReasonCode::SensitiveCert,
            ReasonCode::SensitivePrivateKey,
            ReasonCode::SensitiveCredentials,
        ] {
            assert!(!c.editable());
            assert_eq!(c.default_decision(), Decision::Exclude);
            assert_eq!(c.group(), Group::ExcludedSensitive);
        }
    }

    #[test]
    fn needs_decision_codes_start_undecided_and_are_editable() {
        for c in [ReasonCode::DataFile, ReasonCode::GitignoredNotRebuildable] {
            assert!(c.editable());
            assert_eq!(c.default_decision(), Decision::Undecided);
        }
    }

    #[test]
    fn group_labels_are_distinct() {
        let mut labels: Vec<_> = Group::ALL.iter().map(|g| g.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), Group::ALL.len());
    }
}
