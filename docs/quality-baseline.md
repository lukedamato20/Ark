# Quality and performance evidence baseline

Baseline ID: `windows-ryzen5800x-2026-08-14`

This baseline precedes the roadmap's Phase 7 performance work. It contains synthetic data only;
commands, reports, and retained artifacts must not include conversation content, credentials,
model paths, or runtime bearer tokens.

## Reference workstation

| Property | Value |
|---|---|
| OS | Windows 11 Pro 64-bit, 10.0.26200 |
| CPU | AMD Ryzen 7 5800X, 8 cores / 16 logical processors |
| Memory | 31.9 GiB |
| Workspace disk | 232.3 GiB local volume |
| Rust | 1.95.0 |
| Node / pnpm | Node 25.9.0 / pnpm 10.33.0 |

CI evidence uses pinned Node 22, pnpm 10, stable Rust, and the Windows/macOS/Ubuntu hosted-runner
images recorded by each Actions run. Hardware-dependent thresholds must report the runner image
and compare both an absolute user budget and a relative regression limit.

## Deterministic datasets

`pnpm baseline:check` verifies the committed manifest for a generator that covers:

- 1,000 conversations with deterministic timestamps, Unicode titles, archive, and project fields;
- one 100-message linear thread with append-only revision/branch links;
- one 100,000-character streamed output;
- one maximum-size 20,000-message import.

`pnpm baseline:fixtures` materializes them under ignored `.artifacts/reference-dataset/`. Every
file is synthetic and hash-addressed by `fixtures/reference-dataset-manifest.json`.

## Baseline results and gates

| Surface | Command/tool | Current evidence | Gate |
|---|---|---|---|
| Indexed history/search | `cargo test db::tests::large_history_queries_use_indexes_and_meet_the_100ms_target` | 1,000-row real SQLite fixture passes on the reference workstation | each page and FTS query <100 ms |
| Branch retrieval | `cargo test db::tests::recursive_branch_queries_are_bounded_indexed_and_meet_the_100ms_target` | 250-node branch/path fixture passes | path and descendant query each <100 ms; ≤20,000 nodes |
| Stream persistence | `cargo test db::tests::append_to_message_content_reconstructs_a_large_response_from_batched_checkpoints` | 100,000+ characters reconstruct exactly from 8 KiB checkpoints | no loss; checkpoint rate design ≤4/sec |
| Import ceilings | `cargo test export::tests` | exact 20,000-message and 2,000,000-character boundaries pass; one-over fails | bounded failure before DB mutation |
| Frontend production bundle | `pnpm build` | main 380.48 kB / 120.74 kB gzip; chat 272.37 kB / 82.87 kB gzip; settings 27.30 kB / 7.63 kB gzip | record trend; PERF-005 sets regression threshold |
| Full unit baseline | required validation suite | Rust 162; frontend 7; contract 23 DTOs | zero failures/warnings |

These are pre-Phase-7 baselines, not final performance claims.

## Methods still requiring dedicated instrumentation

- **Desktop startup/interactive:** PERF-001 instruments process start, first window, cached shell,
  and usable composer with monotonic marks; PERF-002 records offline-provider runs.
- **Memory:** PERF-001 samples Ark webview/backend/private bytes after startup, a 100-message
  transcript, and a 100,000-character stream at fixed settle points.
- **TTFT/stream:** existing diagnostics reports TTFT/throughput; PERF-001 adds request IDs,
  checkpoint/event/render spans, and repeatable provider fixtures.
- **Accessibility:** TST-005/006 record axe results, accessibility-tree snapshots, keyboard/focus
  traces, contrast, reduced motion, and the tested viewport.

Each future evidence artifact must contain: baseline ID, commit, dirty-tree flag, UTC timestamp,
OS/runner image, hardware summary, tool versions, dataset manifest hash, command, repetitions,
median/p95 where meaningful, thresholds, and pass/fail. Raw artifacts belong under
`.artifacts/` locally and as bounded CI artifacts; never commit user workspaces.
