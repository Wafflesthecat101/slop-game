# Reviewing a PR — verify it works, not just that it compiles

Lumen is built by many contributors (including agents) in parallel, so a PR
needs to be checked quickly and objectively. This repo ships a **layered,
scriptable review process** that uses Bevy's own tooling to confirm a change
**does what it intends at runtime** — it boots the game headlessly and
introspects the live ECS world over the **Bevy Remote Protocol (BRP)** — on top
of the usual static gate.

> TL;DR: `scripts/review-pr.sh <pr_number>` and read the PASS/FAIL summary.

## Agent runbook — reviewing a PR end to end

If you are an agent asked to *"review PR #N"*, do exactly this:

1. **Run the automated pipeline** from the repo root:
   ```bash
   scripts/review-pr.sh <N>
   ```
   It checks out the PR, runs the static gate, boots the game headlessly with
   BRP, checks the runtime intent, restores your branch, and prints a
   paste-ready Markdown summary. Capture that summary.
   - **Start from a clean working tree.** To review a PR/branch the script must
     switch commits, so it refuses to run (exit 1) if you have uncommitted
     changes — commit or `git stash --include-untracked` first. (`--worktree`
     reviews the current tree in place and skips this.)
   - It verifies `HEAD` actually matches the fetched PR commit before running
     any layer, so a green result always refers to the PR's real code.
   - If the PR changes gameplay and ships its own expectations file (e.g.
     `scripts/expectations.<feature>.txt`), pass it:
     `scripts/review-pr.sh <N> scripts/expectations.<feature>.txt`.
2. **(Optional but encouraged) See it.** If the PR changes anything visual,
   capture a walkthrough so your review shows the world, not just numbers:
   ```bash
   # brp-verify already built + launched the game during step 1's runtime layer,
   # but that process has exited; launch one for screenshots:
   cargo run --features review >/tmp/game.log 2>&1 &
   #   (the scripts export DISPLAY=:97 for you; if launching by hand, see
   #    "Environment notes" below)
   scripts/walkthrough.sh /tmp/walk && kill %1
   ```
   View the PNGs in `/tmp/walk/` and mention what you saw.
3. **Read the diff for quality**, not just correctness. A green gate does **not**
   mean "approve" — still judge design, simplicity, module ownership
   (`CONTRIBUTING.md`), and whether the change matches the issue's intent. See
   the `code-review` skill for the bar.
4. **Post the verdict on the PR.** Combine the pipeline summary, any screenshots
   notes, and your code-quality read into one comment. Use the GitHub API (the
   `gh` CLI is **not** installed here) or the GitHub tools available to you, and
   **disclose that the review was produced by an AI agent**:
   ```bash
   OWNER_REPO=$(git remote get-url origin | sed -E 's#.*[/:]([^/]+/[^/]+?)(\.git)?$#\1#')
   curl -s -X POST \
     -H "Authorization: Bearer $GITHUB_TOKEN" \
     -H "Accept: application/vnd.github+json" \
     "https://api.github.com/repos/$OWNER_REPO/issues/<N>/comments" \
     -d @- <<'JSON'
   { "body": "## Automated review\n\n<paste the review-pr.sh summary table here>\n\n<screenshots / code notes / verdict>\n\n_Review produced by an AI agent (OpenHands)._" }
   JSON
   ```
   Give a clear verdict: **approve**, **approve with nits**, or **request
   changes** (list the blocking items).

Everything below explains *why* and documents each piece in depth.


## Why runtime verification

`cargo build`/`clippy`/`test` prove the code *compiles and its unit tests pass*.
They don't prove the game still *spawns a player and 12 shrines*, still lights
the scene, or that a new plugin actually took effect. Because this is a Bevy
app we can check that directly: run it and ask the running `World` what's in it.

## The layers

The pipeline runs cheapest-first and stops at the first failing layer:

| Layer | What | Tool |
|-------|------|------|
| 1. Static gate | `fmt --check`, `clippy -D warnings`, `test --lib`, wasm `check` | cargo (see [`CONTRIBUTING.md`](CONTRIBUTING.md)) |
| 2. Headless smoke | launches under Xvfb; must boot and run a few seconds with **no panic** and a reachable BRP endpoint | `scripts/brp-verify.sh` |
| 3. Runtime intent | queries the **live ECS** over BRP and asserts the world matches expectations (entity counts, resources present/absent) | `scripts/brp-verify.sh` + an expectations file |

The review tooling itself (`scripts/`, expectations) is snapshotted to a temp
dir **before** the PR is checked out and run from there, so it works even for
PRs/forks whose commit doesn't contain these scripts — you always review with
*your* tooling against the *PR's* game code.

> **Runtime layers need the PR's code to expose BRP** via the opt-in `review`
> feature (below). A PR that predates this tooling (no `review` feature in its
> `Cargo.toml`) can't be introspected at runtime, so layers 2–3 are reported
> **SKIP**, not FAIL, and the overall result reads *PASS (gate only)*. Rebase
> such a PR onto a `main` that has the feature to enable full runtime checks.

### These layers also run automatically in CI

Every push and pull request runs the whole pipeline in GitHub Actions
(`.github/workflows/ci.yml`) — no agent, no human trigger needed:

| Job | Covers |
|-----|--------|
| `test` (Windows/macOS/Linux) + `all-doc-tests` | unit + doc tests (static layer) |
| `lint` | `clippy -D warnings` + `fmt --check` (static layer) |
| `wasm-check` | `cargo check --target wasm32-unknown-unknown` (the GitHub Pages deploy target) |
| `runtime-verify` | builds `--features review`, boots the game headlessly under Xvfb + software Vulkan (lavapipe), and runs `scripts/brp-verify.sh scripts/expectations.default.txt` — i.e. **layers 2–3 above** |

So the runtime intent checks are enforced on every PR as required status
checks, not just when someone runs `review-pr.sh` by hand. Making these checks
**required** in branch protection lets GitHub's native auto-merge merge a PR
only once the live-ECS assertions pass. A gameplay PR that changes the expected
world should update `scripts/expectations.default.txt` (or add its own file and
point the `runtime-verify` job at it) in the same PR.

## The `review` cargo feature (BRP)

Layers 2–3 need the game to expose BRP. That is gated behind an **opt-in
`review` feature** so the shipped game (native + WASM) never carries it:

```toml
# Cargo.toml
[features]
review = ["bevy/bevy_remote", "dep:bevy_brp_extras"]

[dependencies]
bevy_brp_extras = { version = "0.22", optional = true }
```

```rust
// src/lib.rs — inside GamePlugin::build
#[cfg(feature = "review")]
app.add_plugins(bevy_brp_extras::BrpExtrasPlugin::default());
```

`BrpExtrasPlugin` adds the core `RemotePlugin` + `RemoteHttpPlugin` itself
(serving BRP on `127.0.0.1:15702`) and *also* registers the `brp_extras/*`
methods. Build/run it yourself with `cargo run --features review`, then talk to
it with `curl`, [`bevy_brp_mcp`](https://github.com/natepiano/bevy_brp), or any
BRP client. This bevy_remote (0.19) uses the `world.*` JSON-RPC method namespace
(`world.query`, `world.list_resources`, `world.get_resources`, `rpc.discover`, …).

`bevy_brp_extras` adds `brp_extras/*` methods on top of core BRP — most usefully
**in-engine viewport screenshots** (`brp_extras/screenshot`), **input
simulation** (`brp_extras/send_keys`), and a clean **`brp_extras/shutdown`**.
Because the frame is rendered by Bevy itself, screenshots work under the
sandbox's software renderer with no external screenshot tool.

## Seeing the world — screenshots & walkthroughs

Numbers prove structure; screenshots prove it *looks* right. With the game
running under `--features review`:

```bash
# One frame from the player camera:
curl -s -X POST http://127.0.0.1:15702 -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"brp_extras/screenshot","params":{"path":"/tmp/shot.png"}}'

# Walk the player and capture frames along the way:
scripts/walkthrough.sh out/            # writes 00_start.png, 01_KeyW.png, …
```

`scripts/walkthrough.sh` drives the *real* player controller with simulated key
presses (`brp_extras/send_keys` — so terrain-following, sprint FOV, collision,
etc. all apply), logs the camera position after each leg, and screenshots. Set
`STEPS` to script a custom path, e.g.
`STEPS="KeyW:2000 KeyD:1000 KeyW+ShiftLeft:1500" scripts/walkthrough.sh`.
Key names are Bevy `KeyCode` variants (`KeyW`, `ShiftLeft`, `Space`, …).

## Running the review

```bash
# Review an open PR by number (uses the GitHub API; no `gh` CLI needed).
# Set GITHUB_TOKEN for private repos / higher rate limits.
scripts/review-pr.sh 41

# Review a remote branch, or just the current working tree:
scripts/review-pr.sh --branch feat/game-state-machine
scripts/review-pr.sh --worktree
```

The script checks out the PR into a throwaway `review/…` branch, runs the
layers, restores your original branch, and prints a paste-ready Markdown
summary for the PR comment.

### Just the runtime checks

```bash
scripts/brp-verify.sh                       # uses scripts/expectations.default.txt
scripts/brp-verify.sh scripts/expectations.my-pr.txt
BRP_BOOT_SECONDS=10 scripts/brp-verify.sh   # wait longer before asserting
```

## Expectations files — telling the reviewer the *intent*

An expectations file declares, one assertion per line, what the running world
must look like. Blank lines and `#` comments are ignored.

```
entities <fully::qualified::TypePath> <eq|ge|le|gt|lt> <n>
resource_exists <fully::qualified::ResourcePath>
resource_absent  <fully::qualified::ResourcePath>
```

`scripts/expectations.default.txt` holds the baseline invariants for `main`
(one `Camera3d`, twelve beacon `PointLight`s, a sun, sky + ambient resources).

**When your PR changes gameplay, ship its intent as an expectations file.** Copy
the default, adjust it, and point the reviewer at it. Examples:

- *Day/night cycle (#11)* — assert `entities …DirectionalLight ge 1` and that the
  sun's transform/colour changes over time (extend the script with a
  before/after `world.get_components` read).
- *A new plugin that spawns N objects* — `entities <your::Marker> eq N`.
- *Removing an entity kind* — `entities <old::Marker> eq 0`.

### Making your own types visible to BRP

BRP can only see types that are **reflected and registered**. Bevy's built-in
components/resources (`Camera3d`, `PointLight`, `DirectionalLight`,
`ClearColor`, `GlobalAmbientLight`, `Transform`, …) already are, so most intent
can be checked without touching game code. To assert on your *own* type, add:

```rust
#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct Progress { pub lit: u32, pub total: u32 }

// in the plugin:
app.register_type::<Progress>();
```

Keep such reflection additions small and behind normal code (they're cheap and
also help the editor/inspector), or gate them behind `review` if you'd rather
not register them in shipped builds.

## Interpreting a failure

Each layer fails loudly and the pipeline stops there. What a `FAIL` means:

| Symptom (from the output) | Likely cause | What to do |
|---------------------------|--------------|------------|
| `ERROR: working tree has uncommitted changes` | You ran a PR/branch review with a dirty tree | Commit or `git stash --include-untracked`, then re-run (or use `--worktree` to review the tree as-is). |
| `ERROR: after checkout HEAD is … but expected …` | Checkout didn't land on the PR commit | Rare; ensure the tree is clean and the fetch succeeded, then re-run. The script aborts here rather than review the wrong code. |
| `FAIL build --features review` / gate `cargo` error | Compile error, clippy lint, failing unit test, or wasm-only break | Read the quoted error; it's a real regression. Request changes with the file/line. |
| `FAIL game booted (process died)` | Panic or startup error before BRP came up | The script prints the last log lines; look for `panicked at …`. Real crash → request changes. |
| `FAIL BRP endpoint reachable` | Game ran but BRP never answered | Usually the port is busy (another `--features review` game still running — see below) or boot was slow. Kill stray games, or raise `BRP_BOOT_SECONDS`, and re-run. |
| `FAIL no panic during run` | A panic appeared in the log after boot | Inspect the panic; request changes. |
| `FAIL entities … (got X)` | The world doesn't match the expectation | Decide: is it a **real regression** (the PR broke/removed something) → request changes; or are the **expectations stale** (the PR intentionally changed counts and updated/should update the expectations file) → have the PR ship a corrected expectations file. |
| `FAIL entities … (query error …)` | The type path isn't registered/reflected, or is misspelled | Check the fully-qualified path; a *game* type must `#[derive(Reflect)]` + `register_type()` to be queryable (see below). |
| `FAIL resource_exists …` | Resource missing or not reflected | Same as above — real absence vs. not-registered. |

A green run means: it builds on every target, the gate is clean, it boots and
runs without panicking, and the live world matches the declared intent. It does
**not** certify code quality — always pair it with a human/agent read of the
diff.

## Environment notes (this sandbox)

- No GPU/display: `~/workspace/start_bevy_display.sh` starts Xvfb on `:97`;
  the game uses Mesa software rendering (llvmpipe/lavapipe). The scripts start
  it for you and set `DISPLAY`/`XDG_RUNTIME_DIR`.
- **One review game at a time.** BRP binds a fixed port (`15702`), so only one
  `--features review` game can run at once. If a run leaves one behind, stop it
  with `pkill -f 'target/debug/bevy_game'` before the next review.
- Harmless `wgpu_hal::vulkan` / `drm` / ALSA warnings are expected and ignored;
  only real panics/`ERROR`s fail the smoke layer.
- `curl` + `jq` are used for BRP; the first `--features review` build compiles
  `bevy_remote` and its deps once (several minutes), then is fast.
