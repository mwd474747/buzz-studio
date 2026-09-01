# Leftover: `mac-packaged-app-build`

**Status:** needed on any Linux or cloud host. This host cannot produce a
signed, notarized macOS `.app`.
**Does not:** install, start, or replace any live Mac Buzz process.
**Does not:** mint, import, export, print, or rotate keys.
**Does not:** claim a Linux artifact is `Buzz.app`.

## Why this is leftover

The existing Desktop release lane (`scripts/desktop_release.py`,
`.release/desktop-candidate.json`, `just release-desktop`) can package a
content-addressed local-dev production source tree and record boot-time
admission pins. A signed, notarized `Buzz.app` with identifier
`xyz.block.buzz.app` must be built on a Mac worker that already has Apple
signing credentials.

A Boolean `admit_macos_app_artifact(true)` is **not** proof of a signed app.
A bare SHA-256 is also **not** codesign or notarization evidence. `signed`
and `notarized` stay false, and leftover `mac-packaged-app-build` stays
`needed`, until a Mac worker supplies structured codesign identity, Team ID,
notarization/stapling evidence, **and** the artifact digest. Publication is
write-once: the digest directory must not already exist.

## Worker inputs

- Clean `HEAD` recorded as `source_commit` (must equal `git rev-parse HEAD`)
- Complete clean source-tree digest recomputed by `scripts/desktop_release.py`
- Profile pins: `.release/local-dev-production.json`
- Production compile env (Mac worker only; do not apply to a live running app):
  - `BUZZ_DESKTOP_LOCAL_DEV_PRODUCTION=1` (compile-time profile activation)
  - `BUZZ_RELAY_URL=ws://localhost:3300`
  - Owner pin must already be the compiled-in `.release/local-dev-production.json`
    64-hex public key or `sha256:` digest. Env vars are not a Finder-launched
    pin. The in-tree profile does not invent this key; empty pins fail closed.

## Worker outputs

1. Content-addressed release directory written **outside** the source checkout
   and **outside** any DawsOS `reports` / `ops` tree.
2. `artifacts.macos_app.sha256` plus codesign identity, Team ID, and
   notarization/stapling evidence. Digest alone leaves leftover `needed`.
3. Leftover `mac-packaged-app-build` status `satisfied` only when that
   structured evidence is present.
4. Write-once publication (fail if that digest directory already exists).

## Explicit non-goals

- Desktop is optional to `buzz_transport`. Transport is required by Desktop.
  Transport is not failed solely because Desktop is absent.
- No pairing.rs edits.
- No live `#local-dev` private key material in the tree or logs.
- Display prefix `ea840b3e` is not an identity boundary.
