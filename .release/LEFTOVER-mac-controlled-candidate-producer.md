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

Stage 3 recreates the package on the isolated Mac from the approved commit,
then builds from that recreated source or consumes a builder attestation
that Stage 3 authenticates itself. JSON `attestation_class` / `builder`
strings are not builder authority. Linux packages are not transferred as
the Mac production package. Until then, admission refuses `live/` for
self-attested or caller-supplied provenance. The producer hold stays so
that weakness cannot become live today.
