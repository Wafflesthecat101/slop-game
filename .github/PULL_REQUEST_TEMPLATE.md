<!--
Thanks for contributing to Lumen! Keep this PR focused on a single issue.
See docs/CONTRIBUTING.md for the full workflow and the build/test/lint gate.
-->

## Summary

<!-- What does this PR do, and why? Which design pillar / roadmap phase does it serve? -->

Closes #

## Changes

<!-- Bullet the key changes. Note any new module/plugin and the single lib.rs registration line. -->

- 

## How I verified

<!-- Which gate commands passed? Any headless smoke-test notes / screenshots? -->

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -p bevy_game --all-targets -- -D warnings`
- [ ] `cargo test -p bevy_game --lib`
- [ ] `cargo check -p bevy_game --target wasm32-unknown-unknown`

## Design pillar check

- [ ] Upholds P1 (calm, no combat/fail-state), P2 (world reacts), P3 (legible from afar), P4 (cheap by construction)

## Notes for reviewers / parallel work

<!-- Anything other agents should know: shared resources/messages added, files touched, follow-ups. -->
