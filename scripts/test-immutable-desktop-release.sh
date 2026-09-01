#!/usr/bin/env bash
# Deny-case + packaging contract for the immutable local-dev Desktop profile.
# No live #local-dev, no keyring, no Mac install, no Buzz.app pretence.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
packager="$repo_root/scripts/immutable_desktop_release.py"
wrapper="$repo_root/scripts/package-immutable-desktop-release.sh"

fail() { echo "FAIL: $*" >&2; exit 1; }
digest_dir() { printf '%s' "$1" | tr ':' '-'; }

# --- frontendDist production config proof (source files, no app launch) ---
python3 - <<PY
import json
from pathlib import Path
root = Path("$repo_root")
conf = json.loads((root / "desktop/src-tauri/tauri.conf.json").read_text())
assert conf["identifier"] == "xyz.block.buzz.app", conf["identifier"]
assert conf["build"]["frontendDist"] == "../dist", conf["build"]
assert "devUrl" in conf["build"], "devUrl may exist for tauri dev only"
profile = json.loads((root / "desktop/release/local-dev-production.profile.json").read_text())
assert profile["frontend"]["mode"] == "frontendDist"
assert profile["frontend"]["dev_url_active"] is False
assert profile["relay_ws_url"] == "ws://localhost:3300"
assert profile["keyring_service"] == "buzz-desktop"
assert profile["expected_owner_pubkey_prefix"] == "ea840b3e"
assert profile["buzz_transport"] == "optional"
assert profile["mention_seats"] == "not-added"
assert profile["pairing"] == "unchanged"
keyring = (root / "desktop/src-tauri/src/app_state_keyring.rs").read_text()
assert '"buzz-desktop"' in keyring
assert '"buzz-desktop-dev"' in keyring
print("frontendDist + profile pins ok")
PY

# pairing.rs must not be part of this change set / packaging inputs
if git -C "$repo_root" diff --name-only HEAD | grep -qx 'desktop/src-tauri/src/commands/pairing.rs'; then
  fail "pairing.rs is dirty; this work must leave it untouched"
fi

# --- content-addressed package + rollback ---
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
release_root="$tmp/releases"
mkdir -p "$release_root"

commit=$(git -C "$repo_root" rev-parse HEAD)
"$wrapper" "$release_root" >/dev/null
python3 "$packager" verify --release-root "$release_root"

manifest1="$release_root/releases/$(digest_dir "$(cat "$release_root/current")")/manifest.json"
python3 - <<PY
import json
from pathlib import Path
m = json.loads(Path("$manifest1").read_text())
assert m["schema_version"] == 1
assert m["bundle_identifier"] == "xyz.block.buzz.app"
assert m["keyring_service"] == "buzz-desktop"
assert m["relay_ws_url"] == "ws://localhost:3300"
assert m["expected_owner_pubkey_prefix"] == "ea840b3e"
assert m["frontend"]["mode"] == "frontendDist"
assert m["frontend"]["dev_url_active"] is False
assert m["source_commit"] == "$commit"
assert m["artifacts"]["macos_app"] is None
leftovers = {item["id"]: item for item in m["leftovers"]}
assert leftovers["mac-packaged-app-build"]["status"] == "needed"
readme = Path("$manifest1").parent / "artifacts" / "README"
assert "No Buzz.app" in readme.read_text()
assert not list((Path("$manifest1").parent / "artifacts").glob("*.app"))
print("first manifest ok; leftover mac-packaged-app-build recorded")
PY

# Force a second distinct digest by writing a sibling fixture into the release
# root only (not the checkout) is not hashed. Instead, package twice after
# patching a copy of the packager's hashed profile via env is hard. Re-package
# is idempotent; simulate a prior digest by cloning the release tree under a
# fake previous digest so rollback has an exact published target.
current1=$(cat "$release_root/current")
fake_prev="sha256:$(printf 'b%.0s' {1..64})"
# The rollback command requires a real published manifest. Create one by
# copying the first release under a different digest and flipping pointers.
mkdir -p "$release_root/releases/$(digest_dir "$fake_prev")"
cp -a "$release_root/releases/$(digest_dir "$current1")/." \
  "$release_root/releases/$(digest_dir "$fake_prev")/"
python3 - <<PY
import json
from pathlib import Path
p = Path("$release_root/releases/$(digest_dir "$fake_prev")/manifest.json")
m = json.loads(p.read_text())
m["content_digest"] = "$fake_prev"
m["rollback_target"] = None
p.write_text(json.dumps(m, indent=2) + "\n")
Path("$release_root/previous").write_text("$fake_prev\n")
PY

rolled=$(python3 "$packager" rollback --release-root "$release_root")
[[ "$rolled" == "$fake_prev" ]] || fail "rollback did not activate exact previous digest"
[[ "$(cat "$release_root/current")" == "$fake_prev" ]] || fail "current pointer not rolled back"
[[ "$(cat "$release_root/previous")" == "$current1" ]] || fail "previous pointer should keep the old current"

# rollback of unknown digest fails
if python3 "$packager" rollback --release-root "$tmp/empty-root" >/dev/null 2>&1; then
  fail "rollback accepted a missing tree"
fi

# --- forbidden roots ---
if "$wrapper" "$repo_root/desktop/release/out" >/dev/null 2>&1; then
  fail "packager accepted a checkout-relative release root"
fi
if "$wrapper" "$tmp/DawsOS/reports/ops" >/dev/null 2>&1; then
  fail "packager accepted a DawsOS reports/ops root"
fi

# --- wrapper refuses to claim a Mac app ---
out=$("$wrapper" "$tmp/second" || true)
echo "$out" | grep -q 'leftover: mac-packaged-app-build' \
  || fail "wrapper must print the Mac leftover"
echo "$out" | grep -qi 'buzz.app' \
  && echo "$out" | grep -q 'did not produce Buzz.app' \
  || fail "wrapper must state it did not produce Buzz.app"

echo "immutable desktop release contract ok"
