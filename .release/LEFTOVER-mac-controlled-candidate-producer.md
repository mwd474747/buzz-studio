# Leftover: `mac-controlled-candidate-producer`

**Status:** needed. Stage 3 leftover on every host.
**Does not:** hash a caller-supplied `.app` and emit a matching receipt plus
external provenance.
**Does not:** fake a signed `.app` on Linux or macOS.
**Does not:** sign, install, or activate Buzz.app.
**Does not:** invent a Team ID.
**Does not:** write `live/`.

The self-attesting producer is **hard-disabled**. A prebuilt `.app` plus a
generated receipt and a generated `build-provenance.json` only proves those
two claims agree with each other. That is not proof the executable was
compiled from the authenticated source.

`scripts/desktop_release.py local-dev-produce-app` authenticates the source
package, writes this leftover under `candidate/unsigned/`, and stops. It
refuses `--unsigned-app`. It does not manufacture provenance from caller
bytes.

Stage 3 later builds from the exact authenticated source in an isolated Mac
lane, or consumes an independently authenticated builder attestation
(`attestation_class=independent-builder-attestation`,
`builder=isolated-mac-lane`). Until then, admission refuses `live/` for
self-attested or caller-supplied provenance.
