# Performance: targets, runs and results

> **Branch of:** [TECHNICAL_MINDMAP.md](../TECHNICAL_MINDMAP.md) · **Roadmap:** [R4 (V0.1.76)](../ROADMAP.md#v0176-release-candidate), [R10 (V0.2.54)](../ROADMAP.md) · **Status:** V0.2.54. The runs below are the maintainer's: they need release builds, which this repository's rules keep from the automated work, so the results column for V0.2.54 is empty until they are recorded.

RepoMemo promises that a workspace stays quick when it is large. The numbers it promises were set at V0.1.74, measured at V0.1.76 (R4), and are checked again at V0.2.54 with the sources V0.2.x added: **a remote repository of 100,000 files** and **a tracker with 10,000 issues**.

---

## 1. Targets

| What | Target | How it is asked |
|---|---|---|
| Keyword search | p95 under 300 ms | `POST /v1/workspaces/{id}/search` |
| The dashboard | p95 under 500 ms | `GET /v1/dashboard` (three grouped queries) |
| Knowledge map and health | under 2 s on first load, then cached | `GET …/knowledge-map`, `GET …/health` |
| Map and health after one changed file | under 10% of the first, full run | P11 (V0.2.8) |
| A repository sync with nothing changed, and after one changed file | each under 10% of the first sync | R10 |
| Everything above | still holds with 100,000 repository files, or 10,000 issues, in the workspace | R10 |

## 2. The runs

All four are `#[ignore]`d tests that print their timings, so they never run in CI. Use **release** builds: the debug numbers of V0.1.74 were ten times slower and are not a verdict. Each leaves its data in the system temp folder only while it runs.

```powershell
# 50,000 passages: search, the dashboard's queries, the first search that builds the vector index, and (V0.2.154) a page of
# the file list among 100,000 files (REPOMEMO_PERF_LISTED_FILES sets how many)
cargo test -p repomemo-storage --release performance_at_scale -- --ignored --nocapture

# 50,000 passages: the knowledge map and health, first load, cached, and after one changed file (REPOMEMO_PERF_PASSAGES sets the size)
cargo test -p repomemo-api --release map_and_health_at_scale -- --ignored --nocapture

# A repository of 100,000 files: the first sync, a sync with nothing changed, a sync after one changed file,
# then search, the map and health on what it indexed (REPOMEMO_PERF_FILES sets the size)
cargo test -p repomemo-api --release repo_sync_at_scale -- --ignored --nocapture

# A tracker with 10,000 issues through a connector: the passes it takes, the time to index them, then search,
# the dashboard, the map and health (REPOMEMO_PERF_ISSUES sets the size)
cargo test -p repomemo-server --release ten_thousand_issues -- --ignored --nocapture
```

**What the repository run measures, and what it does not.** A remote repository is a bare partial clone on the server that the sync engine reads exactly like a local one, and `repo_sync_at_scale` builds a local repository of that many files (with `git fast-import`) and syncs it: it measures everything after the clone. The transfer of the clone is git's and the host's, and is bounded by `REPOMEMO_REPO_CLONE_MAX_MB` and `REPOMEMO_REPO_CLONE_TIMEOUT_MINUTES`, not by this run.

**What the issues run measures.** A stand-in for GitHub serves the issues the way GitHub pages them. A sync reads at most 30 pages of 100 and resumes from its cursor, so 10,000 issues take at least four passes; the test triggers them one after another and prints each. Issues have no comments here (a comment costs a request each, within the sync's budget of 500).

Each run asserts the targets it can (search p95, the dashboard, the map and health, the 10% rules) and prints the rest.

## 3. Results

Record a run here when it is made: the date, the machine, the commit, the build and the figures.

| Measure | Target | V0.1.76 (R4: release build, 50,000 passages) | V0.2.54 (R10) |
|---|---|---|---|
| Keyword search, p95 | 300 ms | 63 ms | *to record* |
| Meaning search, p95 | | 112 ms | *to record* |
| The dashboard's grouped queries, p95 | 500 ms | 6 ms (figures) | *to record* |
| Knowledge map, first load | 2 s | 0.5 s | *to record* |
| Health, first load | 2 s | 0.4 s | *to record* |
| Map and health after one changed file | 10% of the first | 4.6% (debug build) | *to record* |
| Repository of 100,000 files: first sync | | | *to record* |
| …a sync with nothing changed | 10% of the first | | *to record* |
| …a sync after one changed file | 10% of the first | | *to record* |
| …keyword search p95 / map / health | 300 ms / 2 s / 2 s | | *to record* |
| 10,000 issues: passes and time to read them | | | *to record* |
| …time to index them | | | *to record* |
| …search p95 / dashboard / map / health | 300 ms / 500 ms / 2 s / 2 s | | *to record* |
| A page of 50 in the file list (`GET …/artifacts`), 100,000 files in the workspace, p95 | 100 ms | | *to record (V0.2.154)* |
| SQLite read tuning (V0.2.159): keyword search p95, figures and file list, with `REPOMEMO_SQLITE_CACHE_MB=0` and `REPOMEMO_SQLITE_MMAP_MB=0` (SQLite's defaults) and then with the new defaults (32 and 256) | no slower anywhere | | *to record (V0.2.159): run `performance_at_scale` twice, once for each setting, and write both columns here* |

## 4. Where to look when a number is missed

- **Search or the dashboard**: the query plans in `crates/storage/src/search.rs` and `stats.rs`; the Performance log category (`log_performance_level`) records the duration and size of each.
- **Map and health**: the per-file cache of `crates/api/src/meaning.rs` (P11); a full run recomputes every file's meaning.
- **A repository sync**: `crates/api/src/repo_sync.rs` reads the tree once, compares hashes with the stored files and indexes only what changed; the last author and date come from one `git log` pass (`crates/git`). A sync with nothing changed should read the tree and stop.
- **Issues**: the `MAX_PAGES_PER_SYNC` and `MAX_REQUESTS_PER_SYNC` limits of `crates/api/src/connectors.rs`, and indexing, which is the same queue as for files.
