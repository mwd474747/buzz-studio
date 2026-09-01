# Leftover: `mac-packaged-app-build`

**Status:** needed on any Linux or cloud host. This host cannot produce a
signed, notarized macOS `.app`.
**Does not:** install, start, or replace any live Mac Buzz process.
**Does not:** mint, import, export, print, or rotate keys.
**Does not:** claim a Linux artifact is `Buzz.app`.
**Does not:** claim a signed Mac app exists.

## Why this is leftover

The existing Desktop release lane (`scripts/desktop_release.py`,
`.release/desktop-candidate.json`, `just release-desktop`) can package a
content-addressed local-dev production source tree and record boot-time
admission pins. A signed, notarized `Buzz.app` with identifier
`xyz.block.buzz.app` must be built on a Mac worker that already has Apple
signing credentials.

A Boolean `admit_macos_app_artifact(true)` is **not** proof of a signed app.
Caller-supplied identity, Team ID, or notarization strings are **not**
evidence. `macos_signing_evidence()` must be given a real `.app` path and
must independently recompute the tree digest (files, **symlinks**, and
**modes**), then require **all** of:

- bundle identifier `xyz.block.buzz.app`
- compiled `approved_team_id` / `approved_codesign_identity` (empty fails
  closed; leftover `approved-macos-signing-pin` — do not invent a Team ID)
- bundle executable and version
- source/build receipt matching the source package
- embedded `.release/local-dev-production.json` matching compiled JSON bytes
- `codesign --verify`, Team ID display, Gatekeeper `spctl --assess`, and
  `stapler validate` on macOS

An unrelated Apple-notarized app fails. On Linux this fails closed without
faking macOS tools. Incomplete observations go under `candidate/evidence/`.
`live/` is write-once for a proven candidate only.

## Worker inputs

- Clean `HEAD` recorded as `source_commit` (must equal `git rev-parse HEAD`)
- Complete clean source-tree digest recomputed by `scripts/desktop_release.py`
- Profile pins: `.release/local-dev-production.json`
- Production compile env (Mac worker only; do not apply to a live running app):
  - `BUZZ_DESKTOP_LOCAL_DEV_PRODUCTION=1` (compile-time profile activation)
  - `BUZZ_RELAY_URL=ws://localhost:3300`
  - Owner pin is the ratified compiled-in 64-hex public key and `sha256:` of
    the raw 32-byte key. Env vars are not a Finder-launched pin. Prefix
    `ea840b3e` is display-only.

## Worker outputs

1. Content-addressed release directory written **outside** the source checkout
   and **outside** any DawsOS `reports` / `ops` tree.
2. Independent verification of **this** Buzz.app (not any notarized app).
   On Linux this stays fail-closed.
3. Leftover `mac-packaged-app-build` status `satisfied` and `live/` only
   when that independent verification succeeds. Unsigned observations stay
   in `candidate/evidence/`.
4. Write-once publication (fail if that digest directory already exists).
   Owner pin comes only from the compiled `.release/local-dev-production.json`
   bytes. CLI cannot override it. A forged exact manifest pin against an
   unpinned compiled profile is denied.

## Explicit non-goals

- Desktop is optional to `buzz_transport`. Transport is required by Desktop.
  Transport is not failed solely because Desktop is absent.
- No pairing.rs edits.
- No live `#local-dev` private key material in the tree or logs.
- Display prefix `ea840b3e` is not an identity boundary.
