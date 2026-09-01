# Leftover: `mac-packaged-app-build`

**Status:** needed (this Linux/cloud host cannot produce a signed macOS `.app`)
**Does not:** install, start, or replace any live Mac Buzz process
**Does not:** mint, import, export, print, or rotate keys
**Does not:** send Buzz messages
**Does not:** claim a Linux artifact is `Buzz.app`

## Why this is leftover

The immutable local-dev production profile is source + packaging + deny-case
tests. A signed, notarized `Buzz.app` with bundle identifier
`xyz.block.buzz.app` must be built on a Mac worker that already has Apple
signing credentials.

Do **not** copy, launch, or replace the live Mac Buzz.app while that worker
runs. Preserve Mike’s dirty checkout (including unstaged
`desktop/src-tauri/src/commands/pairing.rs`).

## Worker inputs (already in this branch)

- Source commit recorded in the release manifest `source_commit`
- Profile pins: `desktop/release/local-dev-production.profile.json`
- Manifest schema: `desktop/release/immutable-desktop-manifest.schema.json`
- Packager: `scripts/package-immutable-desktop-release.sh`
- Frontend embed: existing `desktop/src-tauri/tauri.conf.json`
  `build.frontendDist = "../dist"` (not Vite `devUrl`)
- Compile env (Mac only, do not apply to the live running app):
  - `BUZZ_DESKTOP_BUILD_RELAY_URL=ws://localhost:3300`
  - `BUZZ_DESKTOP_IMMUTABLE_PROFILE=1`

## Worker outputs

1. Content-addressed release directory written **outside** the source checkout
   and **outside** any DawsOS `reports` / `ops` tree.
2. `manifest.json` whose `content_digest` matches the tree and whose
   `rollback_target` is the previous published digest (or `null` for the first
   release).
3. A real signed `Buzz.app` recorded under `artifacts.macos_app`.
4. `leftovers` entry `mac-packaged-app-build` flipped to `status: satisfied`.

## Explicit non-goals

- No always-on mention seats
- Desktop remains optional to `buzz_transport`
- No pairing.rs edits (see `PAIRING-IMPACT.md`)
- No live `#local-dev` key material in the tree or logs
