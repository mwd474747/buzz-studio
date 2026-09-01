# Leftover: release-root writer containment residuals

**Status:** writer P1 repair is on this head; residual P2/P3 items below stay
needed. This is **not** a Stage 2 pass. Codex is the verifier.
**Does not:** open `live/` while signing or producer holds remain.
**Does not:** invent a Team ID or codesign identity.
**Does not:** implement Stage 3 live production.

Authority writes now hold one trusted release-root directory descriptor
for the whole operation, never reopen a descendant package pathname, and
never truncate an existing authority inode. Publication is an exclusive
temp (`O_EXCL|O_NOFOLLOW`) under that descriptor, `fsync` of the temp,
descriptor-relative rename (mutable) or no-replace `linkat` (immutable),
then `fsync` of the containing directory.

## Residual debt (carry; do not expand into Stage 3 live work)

1. **Admission/producer write commands do not repeat `forbidden_runtime_root`.**
   `local-dev-package` still checks the release root. `local-dev-admit-app`
   and `local-dev-produce-app` authenticate the current package and write
   under the held descriptor, but they do not repeat that forbidden-root
   check. Repeat it later without opening `live/`.

2. **Future live-manifest construction drops the still-needed rollback leftover.**
   `write_live_if_proven` still refuses `live/` while signing/producer holds
   remain. If it ever constructed a live manifest, that list would omit
   leftover `historical-package-rollback`, which is still needed. Do not
   satisfy rollback or write `live/` here.

3. **`sha256_tree()` omits the root-directory mode for a non-empty bundle.**
   An empty tree records `"."` as a directory. A non-empty walk records
   children only, so two non-empty bundles that differ only in the root
   directory mode can collide. Do not "fix" this by opening `live/` or
   weakening signing/producer holds.

These residuals must not open `live/` while signing/producer holds remain.
