# Leftover: `approved-macos-signing-pin`

**Status:** needed. `approved_team_id` and `approved_codesign_identity` are
empty in `.release/local-dev-production.json`.
**Does not:** invent a Team ID or signing identity.
**Does not:** treat any Apple-notarized app as Buzz.app.
**Does not:** write `live/`.

The real Apple Team ID and Developer ID identity must be compiled in later.
Do not invent them. Until those pins are present, independent `.app`
admission fails closed even if `codesign`, Gatekeeper, and stapler succeed.
