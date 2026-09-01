# Leftover: `nostr-sdk-0.45-migration`

**Status:** needed. Separate work object. Not this PR.
**Does not:** edit `desktop/src-tauri/src/commands/pairing.rs`.
**Does not:** waive RUSTSEC-2026-0231, 0232, or 0257.

## Why this is leftover

RUSTSEC-2026-0243 is the informational *unmaintained* advisory for
`nostr-relay-pool`. The crate's functionality moved into `nostr-sdk` 0.45.0.
Migrating the workspace (and desktop) off `nostr-relay-pool` is an API-level
change across Nostr event/session types.

Pairing (`commands/pairing.rs` and `buzz-core` pairing) consumes those types.
A real 0.45 migration is pairing-adjacent. This PR stops rather than changing
pairing.rs.

## What this PR did instead (honest, not a waive)

- RUSTSEC-2026-0231 (relay AUTH flood) — upgrade `nostr-relay-pool` to `0.44.3`
- RUSTSEC-2026-0232 (unverified relay events) — same `0.44.3` upgrade
- RUSTSEC-2026-0225..0230 (`nostr` crate) — upgrade `nostr` to `0.44.8`
- RUSTSEC-2026-0257 (`webbrowser` Unix `BROWSER` injection) — upgrade to `1.2.4`
- `h2` empty-DATA DoS (RUSTSEC-2026-0258) — upgrade to `0.4.16`

0243 is informational *unmaintained*, not a vulnerability. `deny.toml` records
that fact and points here. It is not an ignore of 0231/0232/0257.

## Follow-up work object

`nostr-sdk-0.45-migration`: move off standalone `nostr-relay-pool` to
`nostr-sdk` 0.45+, with a dedicated pairing impact analysis before any
`pairing.rs` edit.
