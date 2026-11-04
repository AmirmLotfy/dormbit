# Kiro feature probe (Task 4, P00-T04)

Goal: confirm Kiro's documented formats and hook payloads on this machine before real hooks, agents, and skills depend on them.

## Status

| Item                               | Result                                                                                                                                                                                                                                                                                             |
| ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hook files written by `createHook` | `.kiro/hooks/probe-*.json`, schema `version: "v1"`. Kiro reports they become active on the **next session start**.                                                                                                                                                                                 |
| Hook writes need approval          | Confirmed: Kiro asks before every write to `.kiro/hooks/` (a documented always-ask path). One probe (`probe-post-file-save`) was refused by the user and is now denied in user permissions; not retried. PostFileSave payload will be confirmed through the real `format` hook in Task 13 instead. |
| Hook payloads                      | **Pending:** probes fire from the next session. Each run writes `.scratch/hook-probe/<Trigger>-*.txt`. Fields to confirm: `hook_event_name`, `cwd`, `session_id`, `tool_name`, `tool_input` (PreToolUse/PostToolUse); whether PostTaskExec identifies the task.                                    |
| Permissions: workspace scope       | Confirmed: rules whose patterns point outside the workspace are skipped ("Workspace permissions must use workspace-relative patterns"). Moved to user scope.                                                                                                                                       |
| Permissions: deny wins             | Confirmed: `rm -rf .scratch` was blocked by the workspace deny rule.                                                                                                                                                                                                                               |
| Skills format                      | From [docs](https://kiro.dev/docs/skills.md): `.kiro/skills/<name>/SKILL.md`, YAML front matter `name` (must equal folder name) and `description`; custom agents load skills only via `resources: ["skill://..."]`.                                                                                |
| Custom agents                      | From docs: `.kiro/agents/*.md` with front matter; must list steering in `resources`; IDE ignores any agent with a `hooks` field; `.kiro/agents` writes always ask. To verify in Task 13.                                                                                                           |
| Workflows                          | Opt-in (`kiroAgent.workflows.enabled`, Workspace Configuration → Workflows); recipes in `.kiro/workflows/`. Not adopted now: sub-agent review loops cover the plan. Revisit for the release checks.                                                                                                |
| Playwright MCP                     | Launch check deferred to Task 12, when its browsers are installed with approval.                                                                                                                                                                                                                   |
| Powers                             | Catalog reviewed; none installed. See `docs/product/research.md` §5.                                                                                                                                                                                                                               |

## Next step

Start a new Kiro chat session (or reload the window) so the probe hooks load, do one normal turn, then the agent reads `.scratch/hook-probe/`, records field names here, and deletes the probe hooks.
