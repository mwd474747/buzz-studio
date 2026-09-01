# Pairing impact analysis — immutable Desktop release

**Question:** does the Phase 2 immutable production Desktop profile require a
pairing behavior change?

**Answer:** no. Pairing is out of scope.

## Evidence

- `desktop/src-tauri/src/commands/pairing.rs` is **untouched**. It is dirty
  only on Mike’s local checkout and must stay that way.
- Pairing consumes the already-resolved owner identity and the active relay
  URL. This profile pins those values (`buzz-desktop` keyring, owner prefix
  `ea840b3e`, `ws://localhost:3300`) without changing NIP-AB session flow,
  codes, or transport.
- Identity deny-cases (generate-vs-migrate, restart, recovery, locked
  keychain, wrong-identity) live in `immutable_release.rs` as a fail-closed
  policy over *classified* outcomes. They do not remint keys and do not
  alter pairing commands.
- Default `resolve_identity_with_store` is unchanged so ordinary desktop
  and pairing keep their current behavior.

## Separate work object (not this PR)

If a later Mac worker needs a *boot-time* hard fail inside
`resolve_identity_with_store` when `BUZZ_DESKTOP_IMMUTABLE_PROFILE=1`, that
is a new work object (`immutable-profile-boot-gate`). It is **not** pairing,
and it is **not** required to land this packaging + deny-case contract.

Do not fold pairing.rs edits into that object unless a new impact analysis
proves pairing itself is broken.
