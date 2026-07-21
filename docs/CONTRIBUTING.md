# Contributing to Lumen (`slop-game`)

Welcome! This project is built by many contributors — including autonomous AI
agents — working **in parallel**. This guide exists so that a large workforce
can move fast *without stepping on each other*. Read it fully before you start.

See also:
- [`docs/GAME_DESIGN.md`](GAME_DESIGN.md)
- [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) — plugin/module map, diagram, and shared contracts. — the vision, pillars, and roadmap.
- [`AGENTS.md`](../AGENTS.md) — repo-specific build/test/lint notes,
  dependency gotchas, headless-run instructions, and architecture conventions.
  **`AGENTS.md` is authoritative for anything technical; this file is about
  process.**

## 1. Golden rules

1. **Respect the design pillars.** Every change must uphold P1–P4 in
   `GAME_DESIGN.md`. No combat, no fail states, no minimaps, no 60-FPS-budget
   busting. If a task seems to violate a pillar, say so on the issue before
   coding.
2. **One issue → one branch → one PR.** Keep changes focused and scoped to the
   files the issue names.
3. **Keep `main` green.** Run the full gate (below) before opening/updating a
   PR.
4. **The architecture is plugins.** New features are new small plugins or
   additions to an existing one — mirror the style in `src/*.rs`.

## 2. Build / test / lint gate

Run **all** of these before pushing (details & first-run timings in
`AGENTS.md`):

```bash
cargo fmt --check
cargo clippy -p bevy_game --all-targets -- -D warnings
cargo test -p bevy_game --lib
cargo check -p bevy_game --target wasm32-unknown-unknown   # WASM deploy target
```

- Prefer pure, unit-testable functions (like `terrain::height`) so logic can
  be covered by `#[cfg(test)]` tests without spinning up a full Bevy `App`.
- The sandbox has no GPU/display by default; see `AGENTS.md` for the headless
  Xvfb smoke-test recipe. Rely on unit tests + code review for anything that
  can't be automated headlessly.

## 3. Working in a large team (conflict avoidance)

Merge conflicts are the main tax on parallel work. We minimise them by
**module ownership per issue**:

- **Each gameplay concept lives in its own file/plugin.** The current split is
  `world.rs`, `player.rs`, `beacons.rs`, `terrain.rs`, `hud.rs`, composed by
  `lib.rs`. New systems (day/night, lantern, weather, wildlife, audio, menus,
  save) should each get **their own new module + plugin** rather than bloating
  an existing one.
- **Issues are scoped to touch different files.** Before starting, check the
  "Files likely touched" section of your issue. If two open issues both need
  to edit the same file heavily, coordinate on the issues (comment) or
  sequence them via the `blocked` label / milestone ordering.
- **`lib.rs` is a hot file.** It only wires plugins together. When adding a
  plugin, add exactly one line to the `add_plugins((...))` tuple and one `mod`
  line — keep the diff to those two lines to avoid conflicts with other
  agents adding plugins at the same time.
- **Shared contracts go through resources/messages, not cross-module reads.**
  E.g. progress is a `Resource`; "a beacon was lit" is a `Message`. This keeps
  plugins decoupled so they can be built independently. Prefer adding a new
  resource/message over reaching into another plugin's internals.
- **Assets:** put new textures/audio under `assets/<kind>/` with descriptive
  names; never overwrite an existing asset another issue depends on.

## 4. Branch & PR workflow

- Branch from `main`. Name branches `type/short-description`, e.g.
  `feat/day-night-cycle`, `fix/cursor-release`, `docs/roadmap`.
- Commit messages follow **Conventional Commits**:
  `feat:`, `fix:`, `docs:`, `refactor:`, `perf:`, `test:`, `chore:`, `ci:`.
- Open a PR that:
  - references the issue (`Closes #NN`),
  - describes *what* and *why*, and how you verified it (which gate commands
    passed, screenshots/notes if you ran it headlessly),
  - keeps the diff minimal and on-topic.
- Add `Co-authored-by: openhands <openhands@all-hands.dev>` to agent commits.
- Do **not** mark a PR ready-for-review unless the requester asks; leave it as
  a draft with a clear status otherwise.

> Note: `AGENTS.md` records that the maintainer has pre-authorised **direct
> pushes to `main` for this repo**. Even so, prefer PRs for non-trivial
> gameplay changes so work is reviewable and parallelizable; reserve direct
> pushes for docs/infra or when explicitly instructed.

## 5. How to pick up an issue

1. Find an issue labelled `good first issue` (if new) or one in the current
   milestone that is **not** `blocked` and has no open PR.
2. Read its **Context**, **Acceptance criteria**, and **Files likely touched**.
3. Comment to claim it (assign yourself) so others don't duplicate work.
4. Implement the *minimal* change that satisfies the acceptance criteria.
5. Run the gate, open the PR, link the issue.

## 6. Issue quality bar (for whoever files them)

A good issue for this repo has:
- **Context / why** — which pillar & roadmap phase it serves.
- **Acceptance criteria** — a checklist a reviewer can verify.
- **Files likely touched** — so parallel work can be de-conflicted.
- **Dependencies** — "blocked by #NN" if it needs foundational work first.
- **Difficulty & area labels** — so agents can self-select.

## 7. Code style

Follow `<CODE_QUALITY>` conventions already used in the repo: clean, minimal
comments (only for the genuinely non-obvious), imports at the top, small
single-purpose systems, `Single<...>`/filtered queries to keep systems
disjoint. Match the surrounding style; run `cargo fmt`.
