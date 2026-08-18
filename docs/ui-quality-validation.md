# UI quality validation

Ark separates deterministic automated checks from checks that require a packaged native application or a human review on real hardware. This prevents browser-only fixtures from being mistaken for release qualification.

## Automated checks

Run the following from the repository root:

```text
pnpm test:frontend
pnpm test:e2e
pnpm typecheck
pnpm lint
pnpm format:check
pnpm design-tokens:check
pnpm build
```

Vitest uses jsdom, React Testing Library, a fake `ArkClient`, and axe. Serious or critical axe findings are test failures. A negative harness test deliberately renders an inaccessible image and proves that the axe gate detects it. Static token tests cover the named type scale, font stacks, neutral dark surfaces, accent contrast, and reduced-motion classes.

Playwright runs deterministic development fixtures at 390x844, 768x1024, 980x720, and 1280x720 in both light and dark schemes. Every project requests reduced motion. The suite covers keyboard focus, serious/critical axe findings, responsive overflow, Chat/Code switching, New Chat confirmation, and the Models information hierarchy. Visual baselines must be generated with `pnpm test:e2e --update-snapshots`, reviewed in both themes at every viewport, and committed only from the pinned Chromium version in the lockfile. Baselines are not accepted merely because a test runner generated them.

CI installs the pinned Chromium build and runs the component, accessibility, browser, production-web, Rust, and three-platform packaged-Tauri checks. Browser fixtures do not replace packaged-app checks.

## Manual and native release qualification

The following remain native/manual release gates because jsdom and Chromium fixtures cannot establish them reliably:

- Inspect typography, focus rings, truncation, dialogs, drawers, and dense model cards at 100% and 200% zoom on supported Windows, macOS, and Linux releases.
- Review the approved Playwright light/dark images for all four viewport sizes after an intentional visual change.
- Launch each packaged desktop artifact normally and confirm that Ark and every Ark-owned child process open without an unwanted console window. Confirm that developer-mode diagnostics remain available.
- Exercise a real Ollama install/pull/cancel/delete flow, including insufficient disk, unavailable runtime, interrupted transfer, and restart recovery.
- Run a long Chat generation and Ark Code tool sequence, verify cancellation and approval states, and confirm that no activity announcement loops or steals focus.
- Verify credential CRUD with an unlocked platform credential store. Headless or locked stores are an environment failure, not grounds for substituting plaintext storage.
- Verify the configured Brave Search account's current quota/cost and review the pre-approval disclosure using a real request. Provider secrets must remain outside logs, events, persistence, and rendered error text.

Record the operating system, app artifact, display scale, fixture/account prerequisites, result, and evidence link in the release qualification record. A failed or unavailable manual gate must be reported explicitly; it must not be converted into an automated pass.
