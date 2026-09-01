# Leftover: `historical-package-rollback`

**Status:** needed. `local-dev-rollback` is hard-disabled.
**Does not:** mutate `current` or `previous`.
**Does not:** activate a historical package whose manifest can be altered
while its stored source proof is left intact.

A proof-only recompute is not authentication of the complete historical
package. Moving `current` first and discovering the tamper later is a
governance-boundary failure.

Stage 3 recreates the package on the isolated Mac from the approved commit.
It does not transfer a Linux package as the Mac production package and
does not treat JSON `attestation_class` strings as builder authority.

Residual: future live-manifest construction still drops this needed leftover.
Do not satisfy it or write `live/` while signing/producer holds remain.
See `.release/LEFTOVER-release-root-writer-containment.md`.
