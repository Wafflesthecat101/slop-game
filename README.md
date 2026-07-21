# slop-game

A Bevy game project, scaffolded from [NiklasEi/bevy_game_template][template-repo] â€” the most widely-used, actively maintained community template for [Bevy][bevy] games. It ships with ready-to-go CI/CD, including a GitHub Actions workflow that builds the game to WebAssembly and deploys it straight to **GitHub Pages**.

## Repository layout

- **Root** (`Cargo.toml`, `src/`, `assets/`, `mobile/`, `.github/workflows/`, ...) â€” the Bevy game itself, plus all the template's CI/CD, exactly as scaffolded from the template (see [`credits/CREDITS.md`](credits/CREDITS.md)/[License](#license) below), only re-branded (title, links, package ids) for this repo.
- [`blender_landscape/`](blender_landscape) â€” a separate, standalone asset: a procedurally generated Blender landscape (script + `.blend` + rendered PNG), unrelated to the Bevy build.

## Vision & roadmap

*slop-game* is evolving into **Lumen** — a calm, first-person open world that
slowly **reawakens as you carry light back into it**. The full vision, design
pillars, and phased roadmap live in [`docs/GAME_DESIGN.md`](docs/GAME_DESIGN.md).

Building it is a team effort (including autonomous agents working in parallel):

- **[docs/GAME_DESIGN.md](docs/GAME_DESIGN.md)** — vision, pillars, roadmap.
- **[docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)** — how to work in parallel
  without merge conflicts, and the build/test/lint gate.
- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** — plugin/module map and shared contracts for contributors.
- **[docs/REVIEW.md](docs/REVIEW.md)** â€” how PRs are reviewed: an
  agent runbook plus a scriptable pipeline (`scripts/review-pr.sh`) that runs
  the gate, boots the game headlessly, and verifies the live world over the
  Bevy Remote Protocol (with in-engine screenshots).
- **[Development roadmap (master tracker)](https://github.com/Wafflesthecat101/slop-game/issues/38)**
  and the [milestones](https://github.com/Wafflesthecat101/slop-game/milestones)
  — pick an unblocked issue and go.

## Gameplay

The game is a **3D open-world exploration game**. You spawn into a large,
procedurally generated landscape of rolling hills — sandy lowlands, green
midlands and rocky highlands — scattered with textured trees and rocks, and
wrapped in atmospheric distance fog under a warm, low sun. Your goal is to find
and collect all the glowing **beacons**: tall pillars topped with bright,
floating orbs that you can spot from across the map and navigate toward.

- **Mouse** — look around
- **WASD** — walk (with weighty acceleration and subtle head-bob)
- **Shift** — sprint (widens the field of view for a sense of speed)
- **Space** — jump
- **Esc** — release the mouse cursor (click to re-grab)

Trees, rocks and beacon pillars are solid — you walk around them, not through
them. Walk close to a beacon to collect it; the objective counter (top-left) ticks up
and flashes, and announces victory once you've found them all. There's no
timer — it's a calm, curiosity-driven world to explore.

The world (terrain mesh, sky/lighting, scattered scenery) is built in
[`src/world.rs`](src/world.rs); the first-person controller with game-feel
touches (acceleration, head-bob, sprint FOV) lives in
[`src/player.rs`](src/player.rs); the collectible beacon landmarks and the
objective/score loop are in [`src/beacons.rs`](src/beacons.rs); the terrain
shape and biome colouring are a single set of pure functions shared across the
game in [`src/terrain.rs`](src/terrain.rs); and the crosshair, controls hint
and objective counter are in [`src/hud.rs`](src/hud.rs). The ground and object
textures in [`assets/textures/`](assets/textures) are procedurally generated,
seamless, tileable 512x512 PNGs.

## Deploying to GitHub Pages (WASM)

This is already wired up via [`.github/workflows/deploy-page.yaml`](.github/workflows/deploy-page.yaml):

1. Trigger the `deploy-github-page` workflow (Actions tab â†’ "deploy-github-page" â†’ "Run workflow", or via the API).
2. It builds the game with [`trunk`](https://github.com/trunk-rs/trunk) targeting `wasm32-unknown-unknown`, runs `wasm-opt`, and pushes the result to the `gh-pages` branch.
3. Enable [GitHub Pages](https://pages.github.com/) for this repository, sourcing from the `gh-pages` branch.
4. The game will be live at `https://<username>.github.io/<repository>`.

Re-run the workflow any time to publish a newer version. `.github/workflows/ci.yml` also runs `cargo test`/`clippy`/`fmt` on every push across Windows, Linux, and macOS.

## Bevy version

Runs on Bevy **0.19.0** (originally bumped from the template's pinned 0.18.0). The 3D rewrite trimmed the dependency set to what a 3D game needs — Bevy's 3D/UI render features plus `rand` for procedural scattering — and dropped the arcade game's `bevy_kira_audio`, `bevy_asset_loader`, and `webbrowser` deps.

## Original template README

The rest of this document is the original template documentation (setup, icons, mobile/desktop release flows, dev environments), kept for reference:

---

# A Bevy game template

Template for a Game using the awesome [Bevy engine][bevy] featuring out of the box builds for Windows, Linux, macOS, Web (Wasm), Android, and iOS.

# What does this template give you?

* small example ["game"](https://niklasei.github.io/bevy_game_template/)
* easy setup for running the web build using [trunk] (`trunk serve`)
* run the native version with `cargo run`
* workflow for GitHub actions creating releases for Windows, Linux, macOS, and Web (Wasm) ready for distribution
    * the same workflow creates development builds for the mobile platforms (two separate workflows can push to the stores after [some setup](#deploy-mobile-platforms))
    * push a tag in the form of `v[0-9]+.[0-9]+.[0-9]+*` (e.g. `v1.1.42`) to trigger the flow
* CI workflow that checks your application on all native platforms on every push

WARNING: if you work in a private repository, please be aware that macOS and Windows runners cost more build minutes.
**For public repositories the workflow runners are free!**

# How to use this template?

 1. Click "Use this template" on the repository's page
 2. Look for `ToDo` to use your own game name everywhere
 3. [Update the icons as described below](#updating-the-icons)
 4. Start coding :tada:
    * Start the native app: `cargo run`
    * Start the web build: `trunk serve`
        * requires [trunk]: `cargo install --locked trunk`
        * requires `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
        * this will serve your app on `8080` and automatically rebuild + reload it after code changes
    * Start the android app: `cargo apk run -p mobile`
        * requires following the instructions in the [bevy example readme for android setup][android-instructions]
    * Start the iOS app (see the [bevy example readme for ios setup instructions][ios-instructions])
        * Install Xcode through the app store
        * Launch Xcode and install the iOS simulator (check the box upon first start, or install it through `Preferences > Platforms` later)
        * Install the iOS and iOS simulator Rust targets with `rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim`
        * run `make run` inside the `/mobile` directory

You should keep the `credits` directory up to date. The release workflow automatically includes the directory in every build.

### Updating the icons
 1. Replace `build/macos/icon_1024x1024.png` with a `1024` times `1024` pixel png icon and run `create_icns.sh` or `create_icns_linux.sh` if you use linux (make sure to run the script inside the `build/macos` directory) - _Note: `create_icns.sh` requires a mac, and `create_icns_linux.sh` requires imagemagick and png2icns_
 2. Replace `build/windows/icon.ico` (used for windows executable and as favicon for the web-builds)
    * You can create an `.ico` file for windows by following these steps:
       1. Open `macos/AppIcon.iconset/icon_256x256.png` in [Gimp](https://www.gimp.org/downloads/)
       2. Select the `File > Export As` menu item.
       3. Change the file extension to `.ico` (or click `Select File Type (By Extension)` and select `Microsoft Windows Icon`)
       4. Save as `build/windows/icon.ico`
 3. Replace `build/android/res/mipmap-mdpi/icon.png` with `macos/AppIcon.iconset/icon_256x256.png`, but rename it to `icon.png`

### Deploy web build to GitHub pages

 1. Trigger the `deploy-github-page` workflow
 2. Activate [GitHub pages](https://pages.github.com/) for your repository
     1. Source from the `gh-pages` branch (created by the just executed action)
 3. After a few minutes your game is live at `http://username.github.io/repository`

To deploy newer versions, just run the `deploy-github-page` workflow again.

# Deploy mobile platforms

For general info on mobile support, you can take a look at [one of my blog posts about mobile development with Bevy][mobile_dev_with_bevy_2] which is relevant to the current setup.

## Android

Currently, `cargo-apk` is used to run the development app. But APKs can no longer be published in the store and `cargo-apk` cannot produce the required AAB. This is why there is setup for two android related tools. In [`mobile/Cargo.toml`](./mobile/Cargo.toml), the `package.metadata.android` section configures `cargo-apk` while [`mobile/manifest.yaml`](./mobile/manifest.yaml) configures a custom fork of `xbuild` which is used in the `release-android-google-play` workflow to create an AAB.

There is a [post about how to set up the android release workflow][workflow_bevy_android] on my blog.

## iOS

The setup is pretty much what Bevy does for the mobile example.

There is a [post about how to set up the iOS release workflow][workflow_bevy_ios] on my blog.

# Removing mobile platforms

If you don't want to target Android or iOS, you can just delete the `/mobile`, `/build/android`, and `/build/ios` directories.
Then delete the `[workspace]` section from `Cargo.toml`.

# Development environments

## Nix Support

nixgl is only used on non-NixOS Linux systems;
when running there we need to use the `--impure` flag:

```
nix develop --impure
```

If using nixgl, then .e.g. `gl cargo run`, other use
`cargo` as usual.

# Getting started with Bevy

You should check out the Bevy website for [links to resources][bevy-learn] and the [Bevy Cheat Book] for a bunch of helpful documentation and examples. I can also recommend the [official Bevy Discord server][bevy-discord] for keeping up to date with the development and getting help from other Bevy users.

# Known issues

Audio in web-builds can have issues in some browsers. This seems to be a general performance issue and not due to the audio itself (see [bevy_kira_audio/#9][firefox-sound-issue]).

# License

This project is licensed under [CC0 1.0 Universal](LICENSE) except some content of `assets` and the Bevy icons in the `build` directory (see [Credits](credits/CREDITS.md)). Go crazy and feel free to show me whatever you build with this ([@nikl_me][nikl-twitter] / [@nikl_me@mastodon.online][nikl-mastodon] ).

[template-repo]: https://github.com/NiklasEi/bevy_game_template
[bevy]: https://bevyengine.org/
[bevy-learn]: https://bevyengine.org/learn/
[bevy-discord]: https://discord.gg/bevy
[nikl-twitter]: https://twitter.com/nikl_me
[nikl-mastodon]: https://mastodon.online/@nikl_me
[firefox-sound-issue]: https://github.com/NiklasEi/bevy_kira_audio/issues/9
[Bevy Cheat Book]: https://bevy-cheatbook.github.io/introduction.html
[trunk]: https://github.com/trunk-rs/trunk
[android-instructions]: https://github.com/bevyengine/bevy/blob/latest/examples/README.md#setup
[ios-instructions]: https://github.com/bevyengine/bevy/blob/latest/examples/README.md#setup-1
[mobile_dev_with_bevy_2]: https://www.nikl.me/blog/2023/notes_on_mobile_development_with_bevy_2/
[workflow_bevy_android]: https://www.nikl.me/blog/2023/github_workflow_to_publish_android_app/
[workflow_bevy_ios]: https://www.nikl.me/blog/2023/github_workflow_to_publish_ios_app/
