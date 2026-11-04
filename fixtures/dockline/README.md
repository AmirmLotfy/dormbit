# Dockline fixture

A fictional invoicing app for a marine supply shop, used by every demo and golden test
(foundation Requirements 18 and 19). Nothing in it is real: the two authors use `.invalid`
addresses, `.env` values all say `fake`, and agent messages are placeholders.

| File | What it is |
| --- | --- |
| `dockline-invoices.tar` | ustar archive: `dockline-invoices/` (the project with its `.git`) and `agent-stores/` |
| `dockline-invoices.sidecar.json` | expected answers: Git snapshot, agent session metadata, SHA-256 of every file |
| `generator/` | the `dockline-gen` crate that builds both files |

The project has 24 commits (17 on `main`, 7 more on `feature/pdf-export`, the last one
`WIP: pdf totals`), HEAD on `feature/pdf-export`, one stash (`rate limits`), 2 modified files,
1 untracked note, and ignored `.env`, `data/dev.sqlite`, `dist/`, and `node_modules/`.

## Regenerate and check

```sh
cargo run -p dockline-gen --locked -- --out fixtures/dockline   # rebuild after an intended change
cargo test -p dockline-gen --locked                             # Fixture_Check and determinism
sh scripts/check-dockline.sh                                    # extract and inspect, gitleaks
```

The generator never reads the clock, the machine's Git config, or `HOME`: dates are fixed,
every commit and the stash use explicit signatures, and Git runs with `HOME` and
`XDG_CONFIG_HOME` set to an empty temp folder. The only entry that differs between runs is
`dockline-invoices/.git/index`, which stores file stat times.
