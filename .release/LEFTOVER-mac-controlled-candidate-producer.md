# Leftover: `mac-controlled-candidate-producer`

**Status:** needed on Linux and on any host that has not run the controlled
Mac producer.
**Does not:** fake a signed `.app` on Linux.
**Does not:** sign, install, or activate Buzz.app.
**Does not:** invent a Team ID.

The producer embeds the compiled `.release/local-dev-production.json` and a
generated source receipt **before** any signing step, then binds the
executable digest to authenticated build provenance written **outside** the
bundle (`candidate/unsigned/build-provenance.json`). Signed self-declared
resources inside the `.app` are not sufficient proof the executable was
compiled from that source/profile.

`scripts/desktop_release.py local-dev-produce-app` is that producer. On
Linux it records this leftover and stops.
