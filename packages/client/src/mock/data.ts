// The MockClient's in-memory dataset, shaped like the Dockline fixture: one Read grant,
// dockline-invoices plus two small projects, a plan, a capsule, a verification report, a
// restore report, and a brief. `dataset()` returns a fresh copy for every MockClient.
import type {
  AdapterInfo,
  Brief,
  CapsuleRef,
  CapturePlan,
  Finding,
  PlanItem,
  ProjectRecord,
  ScopeGrant,
  Settings,
  VerificationReport,
} from "@dormbit/schema";

export const IDS = {
  scope: "5c1f3a2e-8d4b-4c6a-9f10-6d2a1b000001",
  dockline: "5c1f3a2e-8d4b-4c6a-9f10-6d2a1b000002",
  harbor: "5c1f3a2e-8d4b-4c6a-9f10-6d2a1b000003",
  paperweight: "5c1f3a2e-8d4b-4c6a-9f10-6d2a1b000004",
  plan: "5c1f3a2e-8d4b-4c6a-9f10-6d2a1b000005",
  capsule: "5c1f3a2e-8d4b-4c6a-9f10-6d2a1b000006",
} as const;

export interface MockData {
  scopes: ScopeGrant[];
  projects: ProjectRecord[];
  findings: Map<string, Finding[]>;
  plans: Map<string, CapturePlan>;
  capsules: CapsuleRef[];
  verification: VerificationReport;
  briefs: Map<string, Brief>;
  settings: Settings;
  adapters: AdapterInfo[];
}

const stack = (
  id: string,
  rule: string,
  statement: string,
  path: string,
  detail: string,
): Finding => ({
  id,
  rule,
  statement,
  status: "confirmed",
  evidence: [{ path, detail }],
});

const noPreservation = { last_capsule: null, last_verified_at: null, last_verified_restore: null };

function projects(): ProjectRecord[] {
  return [
    {
      id: IDS.dockline,
      scope_id: IDS.scope,
      rel_root: "clients/dockline/dockline-invoices",
      name: { value: "dockline-invoices", source: "Cargo.toml" },
      stacks: [
        stack(
          "F-01",
          "stack.dependency",
          "Rust crate",
          "Cargo.toml",
          '[package] name = "dockline-invoices"',
        ),
        stack(
          "F-02",
          "stack.dependency",
          "TypeScript web client",
          "web/package.json",
          "/devDependencies/typescript",
        ),
      ],
      git: {
        head: {
          branch: "feature/csv-export",
          detached: false,
          commit: "9f2c41ab7d0e5f3c8a61b2d94e7c05a1f3b8d260",
          time: "2026-04-02T17:38:55Z",
          summary: "wip: round line totals before tax",
        },
        upstream: { ahead: 3, behind: 0, as_of_last_fetch: true },
        status: { modified: 2, staged: 1, untracked: 1, deleted: 0, conflicted: 0 },
        stash_count: 1,
        worktree_external: false,
        submodules: [],
        lfs_detected: false,
        risky_config: [],
        remotes_redacted: ["https://github.com/dockline/dockline-invoices.git"],
      },
      activity: { latest_mtime: "2026-04-02T17:41:09Z", scanned: 8623, partial: false },
      size: { total: 1329446912, rebuildable: 1281232896, partial: false },
      docs: ["README.md", "NOTES.md"],
      manifests: ["Cargo.toml", "web/package.json"],
      agent_artifacts: ["AGENTS.md"],
      marks: { dormant: true },
      preservation: {
        last_capsule: IDS.capsule,
        last_verified_at: "2026-11-02T10:12:00Z",
        last_verified_restore: null,
      },
    },
    {
      id: IDS.harbor,
      scope_id: IDS.scope,
      rel_root: "harbor-sync",
      name: { value: "harbor-sync", source: "go.mod" },
      stacks: [
        stack(
          "F-21",
          "stack.dependency",
          "Go module",
          "go.mod",
          "module github.com/ann/harbor-sync",
        ),
      ],
      git: {
        head: {
          branch: "feature/retry",
          detached: false,
          commit: "4be1c09d2a7f6e1b3c5d8e9f0a1b2c3d4e5f6a7b",
          time: "2025-10-28T09:02:11Z",
          summary: "retry uploads with backoff",
        },
        upstream: { ahead: 0, behind: 0, as_of_last_fetch: true },
        status: { modified: 1, staged: 0, untracked: 0, deleted: 0, conflicted: 0 },
        stash_count: 0,
        worktree_external: false,
        submodules: [],
        lfs_detected: false,
        risky_config: [],
        remotes_redacted: ["git@github.com:ann/harbor-sync.git"],
      },
      activity: { latest_mtime: "2025-10-28T09:05:40Z", scanned: 412, partial: false },
      size: { total: 12902400, rebuildable: 9437184, partial: false },
      docs: ["README.md"],
      manifests: ["go.mod"],
      agent_artifacts: [],
      marks: { dormant: false },
      preservation: noPreservation,
    },
    {
      id: IDS.paperweight,
      scope_id: IDS.scope,
      rel_root: "sandbox/paperweight",
      name: { value: "paperweight", source: "pyproject.toml" },
      stacks: [
        stack(
          "F-31",
          "stack.dependency",
          "Python package",
          "pyproject.toml",
          '[project] name = "paperweight"',
        ),
      ],
      git: {
        head: {
          branch: "main",
          detached: false,
          commit: "c0ffee51a2b3c4d5e6f708192a3b4c5d6e7f8091",
          time: "2024-11-12T21:14:03Z",
          summary: "first working parser",
        },
        upstream: null,
        status: { modified: 0, staged: 0, untracked: 0, deleted: 0, conflicted: 0 },
        stash_count: 0,
        worktree_external: false,
        submodules: [],
        lfs_detected: false,
        risky_config: [],
        remotes_redacted: [],
      },
      activity: { latest_mtime: "2024-11-12T21:20:00Z", scanned: 57, partial: false },
      size: { total: 3145728, rebuildable: 1048576, partial: false },
      docs: ["README.md"],
      manifests: ["pyproject.toml"],
      agent_artifacts: [],
      marks: { dormant: false },
      preservation: noPreservation,
    },
  ];
}

const file = (path: string, size: number, reason: string): PlanItem => ({
  path,
  kind: "file",
  size,
  decision: "include",
  reason,
  editable: true,
  warning: null,
});

/** The dockline plan. Totals and the digest are filled in by the MockClient. */
export function docklinePlanItems(): PlanItem[] {
  return [
    file("Cargo.toml", 1184, "manifest"),
    file("src/lib.rs", 18204, "source"),
    file("src/invoice.rs", 26930, "source"),
    file("src/export/csv.rs", 9411, "source, changed since the last commit"),
    file("src/tax.rs", 7302, "source, changed since the last commit"),
    file("web/package.json", 1210, "manifest"),
    file("web/src/App.tsx", 14880, "source"),
    file("README.md", 4120, "documentation"),
    file("NOTES.md", 2310, "untracked note"),
    file("AGENTS.md", 1904, "agent instructions"),
    {
      path: "docs/latest.md",
      kind: "symlink",
      size: 0,
      decision: "include",
      reason: "symlink, recorded but not followed",
      editable: false,
      warning: "restored as a note, not a link",
    },
    {
      path: ".env",
      kind: "file",
      size: 212,
      decision: "exclude",
      reason: "sensitive: environment secrets",
      editable: false,
      warning: "never captured",
    },
    {
      path: "fixtures/sample-invoices.pdf",
      kind: "file",
      size: 4812004,
      decision: "exclude",
      reason: "large binary",
      editable: true,
      warning: null,
    },
  ];
}

export function dataset(): MockData {
  const records = projects();
  const docklineStacks = records.find((p) => p.id === IDS.dockline)?.stacks ?? [];
  const capsule: CapsuleRef = {
    id: IDS.capsule,
    project_name: "dockline-invoices",
    created_at: "2026-11-02T10:09:31Z",
    size: 46802944,
    file_count: 212,
  };
  return {
    scopes: [
      {
        id: IDS.scope,
        kind: "Read",
        root_display: "~/dev",
        granted_at: "2026-11-02T09:58:00Z",
        via: "NativePicker",
      },
    ],
    projects: records,
    findings: new Map([
      [
        IDS.dockline,
        [
          ...docklineStacks,
          {
            id: "F-12",
            rule: "git.unpushed",
            statement: "feature/csv-export is 3 commits ahead of origin",
            status: "confirmed",
            evidence: [
              {
                path: ".git/refs/heads/feature/csv-export",
                detail: "ahead 3, behind 0 at last fetch",
              },
            ],
          },
          {
            id: "F-13",
            rule: "git.stash",
            statement: "One stash: wip: rounding fix",
            status: "confirmed",
            evidence: [{ path: ".git/refs/stash", detail: "stash@{0}" }],
          },
          {
            id: "F-14",
            rule: "work.in_progress",
            statement: "Work stopped partway through a rounding fix in src/tax.rs",
            status: "inferred",
            evidence: [
              { path: "src/tax.rs", detail: "modified, not committed" },
              { path: "NOTES.md", detail: "lines 3-5: TODO round per line, not per invoice" },
            ],
          },
        ],
      ],
    ]),
    plans: new Map(),
    capsules: [capsule],
    verification: {
      capsule_id: IDS.capsule,
      file_sha256: "e0a0323b851f2bce71c80953c42542733e6b3a07b8311b36c41cc19649c6e374",
      manifest_sha256: "3f9a1c0be2d74a5f86c1e0d9b7a3f2e14c5d6b7a8e9f0a1b2c3d4e5f6a7b8c9d",
      checks: [
        { id: "marker", status: "pass", detail: "dormbit-capsule/1" },
        { id: "manifest", status: "pass", detail: "schema 1" },
        { id: "objects", status: "pass", detail: "212 of 212 objects match" },
        { id: "end", status: "pass", detail: "END trailer digest matches" },
      ],
      failed_files: [],
      registry_match: "Match",
      status: "pass",
    },
    briefs: new Map([
      [
        IDS.dockline,
        {
          sections: [
            {
              title: "Where you stopped",
              body: "You stopped on `feature/csv-export`, 3 commits ahead of origin, partway through a rounding fix in `src/tax.rs`. [F-12] [F-14]",
            },
            {
              title: "Before you start",
              body: "Restore the stash `wip: rounding fix` and read `NOTES.md` lines 3 to 5. [F-13]",
            },
          ],
          outcomes: {
            files: "preserved",
            context: "preserved",
            session: "partial",
            build: "not_captured",
          },
          evidence_index: [
            { path: ".git/refs/heads/feature/csv-export", detail: "F-12" },
            { path: ".git/refs/stash", detail: "F-13" },
            { path: "src/tax.rs", detail: "F-14" },
          ],
        },
      ],
    ]),
    settings: { reduce_motion: false, capsule_max_mb: 4096 },
    adapters: [
      { id: "claude-code", name: "Claude Code", support: "supported", granted: false },
      { id: "codex", name: "Codex", support: "supported", granted: false },
      { id: "kiro-export", name: "Kiro export", support: "supported", granted: false },
      { id: "cursor", name: "Cursor", support: "unsupported", granted: false },
    ],
  };
}
