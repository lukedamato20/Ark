# Ark Code behavioral parity checklist — v1

- Version: 1.0
- Evidence date: 2026-08-17
- Engineering review: complete
- Product review: pending
- Scope: interaction behavior, not visual imitation or provider-specific capabilities

## Reference behavior

This checklist uses current first-party descriptions of [Claude Code's interactive/resumable
workflow](https://docs.anthropic.com/en/docs/claude-code/cli-usage), Claude Code's explicit
tool-permission controls, and the [Codex app's project threads, inline change review, and isolated
worktrees](https://openai.com/index/introducing-the-codex-app/). It also uses OpenAI's description
of the [Codex agent loop](https://openai.com/index/unrolling-the-codex-agent-loop/) as the reference
for automatic model/tool/observation iteration.

## Required interaction parity

| Behavior | Ark implementation evidence | Result |
| --- | --- | --- |
| Start with a natural-language outcome in a repository-scoped conversation | Ark Code session list, persistent composer, Project/Repository binding | Meets |
| Continue or resume the same conversation | Durable sessions/runs, parent-run chain, selected session and draft restoration | Meets |
| Agent investigates and iterates without a manual “run each tool” workflow | Controlled automatic provider/tool loop with bounded steps/time/tokens | Meets |
| Show incremental assistant progress in the conversation | Durable streaming text checkpoints and typed progress state | Meets |
| Keep tool activity causally ordered with conversation turns | Sequenced run, step, invocation, observation, and event records | Meets |
| Review proposed changes where they occur in the thread | Typed inline edit/checkpoint/rollback cards with exact previews | Meets |
| Review commands before execution | Typed inline fixed-command card showing executable, arguments, cwd, environment policy, and timeout | Meets |
| Approve, reject, or revise without leaving the conversation | Per-invocation buttons plus composer-focused revision path | Meets |
| Stop active work and provide new direction | Killable cancellation and Stop & steer child-run flow | Meets |
| Ask a clarification and accept the answer through the same composer | Typed clarification item and deterministic composer focus | Meets |
| Retry/continue terminal work | Explicit retry instruction and causal child-run continuation | Meets |
| Inspect repository, changes, output, and run diagnostics without replacing the thread | Optional closable supporting panes | Meets |
| Preserve state while switching between general chat and coding work | Separate Ark Chat/Ark Code stores, authority, active selection, draft, focus, and scroll | Meets |
| Remain usable over long histories and small model contexts | Visible compaction plus bounded incremental timeline rendering with stable prepend anchoring | Meets |

## Intentional Ark differences

These differences preserve Ark's local-first, provider-independent security model and are not
parity defects:

1. Ark does not offer a generic shell or “dangerously skip permissions” mode. Models select only a
   local-user-defined build/test/lint command ID; every execution still receives an exact per-use
   approval.
2. Ark creates a private managed clone and a dedicated `ark/session/<session-id>` branch. Agent work
   never shares or mutates the user's active checkout, dirty tree, index, or branch.
3. Ark requires explicit approval for every edit, checkpoint, rollback, and command. There is no
   “approve all” interaction; each approval is bound to call, preview, and precondition hashes.
4. Ark treats repository files, tool results, model output, and command output as untrusted data.
   They cannot create trusted timeline controls or enter the system-instruction channel.
5. Ark keeps local models and prompted tool-calling providers in the same interaction model. When a
   model lacks a required capability or usable context window, the limitation is shown explicitly.
6. Ark exposes context budget, compaction, recovery outcome, and durable lifecycle diagnostics more
   directly than the reference products because offline recovery must be independently auditable.

## Product review record

Product must confirm that the implemented flow feels like a coding conversation—not a repository
dashboard—and accept the intentional differences above before CODE-007 is marked Complete.

- Reviewer: pending
- Decision: pending
- Date: pending
- Notes: pending
