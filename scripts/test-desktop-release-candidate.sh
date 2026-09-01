#!/usr/bin/env bash
set -euo pipefail
export PYTHONDONTWRITEBYTECODE=1

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cp "$repo_root/scripts/desktop_release.py" "$tmp/desktop_release.py"

git -C "$tmp" init -q
git -C "$tmp" config user.name test
git -C "$tmp" config user.email test@example.com
mkdir -p "$tmp/scripts" "$tmp/desktop/src-tauri" "$tmp/crates/buzz-core" "$tmp/.release"
mv "$tmp/desktop_release.py" "$tmp/scripts/desktop_release.py"
cp "$repo_root/.release/local-dev-production.json" "$tmp/.release/local-dev-production.json"
printf '{"version":"1.0.0"}\n' > "$tmp/desktop/package.json"
printf '{"version":"1.0.0"}\n' > "$tmp/desktop/src-tauri/tauri.conf.json"
printf '[package]\nversion = "1.0.0"\n' > "$tmp/desktop/src-tauri/Cargo.toml"
echo '# Changelog' > "$tmp/CHANGELOG.md"
echo first > "$tmp/desktop/feature"
git -C "$tmp" add .
git -C "$tmp" commit -qm 'feat: first desktop change'
git -C "$tmp" -c tag.gpgSign=false tag v1.0.0
echo second >> "$tmp/desktop/feature"
git -C "$tmp" commit -qam 'fix: desktop fix'
echo policy > "$tmp/POLICY.md"
git -C "$tmp" add POLICY.md
git -C "$tmp" commit -qm 'docs: repository policy'
base=$(git -C "$tmp" rev-parse HEAD)
(
  cd "$tmp"
  scripts/desktop_release.py generate 1.0.1 --base "$base" --repo block/buzz
  python3 - <<'PY'
import json
for path in ('desktop/package.json', 'desktop/src-tauri/tauri.conf.json'):
    data=json.load(open(path)); data['version']='1.0.1'; open(path,'w').write(json.dumps(data)+'\n')
p='desktop/src-tauri/Cargo.toml'; open(p,'w').write('[package]\nversion = "1.0.1"\n')
PY
  rm -f msg
  git add .
  cat >msg <<'EOF'
chore(release): release Buzz Desktop version 1.0.1

Co-authored-by: Test Automation <test@example.com>
EOF
  git -c user.name=Wes -c user.email=wesbillman@users.noreply.github.com commit -q -s -F msg
  rm msg
  scripts/desktop_release.py validate --version 1.0.1 --repo block/buzz
  grep -Fq '### Other repository changes' CHANGELOG.md
  grep -Fq "$(git rev-parse HEAD~1)" CHANGELOG.md
  grep -Fq "$(git rev-parse HEAD~2)" CHANGELOG.md

  # Metadata cannot lie about the prior release boundary.
  cp .release/desktop-candidate.json metadata.json
  python3 - <<'PY'
import json
p='.release/desktop-candidate.json'; d=json.load(open(p)); d['previous_tag']=None; open(p,'w').write(json.dumps(d)+'\n')
PY
  if scripts/desktop_release.py validate --version 1.0.1 --repo block/buzz >/dev/null 2>&1; then
    echo "validator accepted a forged previous release tag" >&2
    exit 1
  fi
  mv metadata.json .release/desktop-candidate.json
)

# An initial release must account for the root commit, not silently omit it.
initial=$(mktemp -d)
cp "$repo_root/scripts/desktop_release.py" "$initial/desktop_release.py"
git -C "$initial" init -q
git -C "$initial" config user.name test
git -C "$initial" config user.email test@example.com
mkdir -p "$initial/scripts" "$initial/desktop/src-tauri" "$initial/.release"
mv "$initial/desktop_release.py" "$initial/scripts/desktop_release.py"
cp "$repo_root/.release/local-dev-production.json" "$initial/.release/local-dev-production.json"
printf '{"version":"0.1.0"}\n' > "$initial/desktop/package.json"
printf '{"version":"0.1.0"}\n' > "$initial/desktop/src-tauri/tauri.conf.json"
printf '[package]\nversion = "0.1.0"\n' > "$initial/desktop/src-tauri/Cargo.toml"
printf '# Changelog\n' > "$initial/CHANGELOG.md"
echo root > "$initial/ROOT.md"
git -C "$initial" add .
git -C "$initial" commit -qm 'feat: root release content'
root_sha=$(git -C "$initial" rev-parse HEAD)
(cd "$initial" && scripts/desktop_release.py generate 0.1.0 --base "$root_sha" --repo block/buzz)
grep -Fq "$root_sha" "$initial/CHANGELOG.md"
rm -rf "$initial"

# Local-dev production profile: complete source-tree hash, HEAD proof,
# write-once publication, recompute-on-verify, authenticated rollback.
local_dev=$(mktemp -d)
release_root=$(mktemp -d)
trap 'rm -rf "$tmp" "$local_dev" "$release_root"' EXIT
cp "$repo_root/scripts/desktop_release.py" "$local_dev/desktop_release.py"
git -C "$local_dev" init -q
git -C "$local_dev" config user.name test
git -C "$local_dev" config user.email test@example.com
mkdir -p "$local_dev/scripts" "$local_dev/desktop/src-tauri" "$local_dev/.release" \
  "$local_dev/crates/buzz-core" "$local_dev/docs"
mv "$local_dev/desktop_release.py" "$local_dev/scripts/desktop_release.py"
cp "$repo_root/.release/local-dev-production.json" "$local_dev/.release/local-dev-production.json"
printf '{"$schema":"https://schema.tauri.app/config/2","productName":"Buzz","identifier":"xyz.block.buzz.app","build":{"frontendDist":"../dist","devUrl":"http://localhost:1420"}}\n' \
  > "$local_dev/desktop/src-tauri/tauri.conf.json"
echo 'alpha' > "$local_dev/crates/buzz-core/lib.rs"
echo 'doc' > "$local_dev/docs/NOTE.md"
echo 'readme' > "$local_dev/README.md"
git -C "$local_dev" add .
git -C "$local_dev" commit -qm 'feat: complete source tree for local-dev digest'
(
  cd "$local_dev"
  if scripts/desktop_release.py local-dev-package --release-root "$release_root" \
      --source-commit aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >/dev/null 2>&1; then
    echo "packager accepted an arbitrary 40-char source_commit" >&2
    exit 1
  fi
  echo dirty >> README.md
  if scripts/desktop_release.py local-dev-package --release-root "$release_root" >/dev/null 2>&1; then
    echo "packager hashed a dirty source tree" >&2
    exit 1
  fi
  git checkout -- README.md

  owner=ea840b3e$(python3 -c 'print("ab"*28)')
  if scripts/desktop_release.py local-dev-package --release-root "$release_root" \
      --owner-pubkey "$owner" >/dev/null 2>&1; then
    echo "packager accepted a CLI owner pin override" >&2
    exit 1
  fi
  scripts/desktop_release.py local-dev-package --release-root "$release_root"
  scripts/desktop_release.py local-dev-verify --release-root "$release_root"
  if scripts/desktop_release.py local-dev-package --release-root "$release_root" >/dev/null 2>&1; then
    echo "packager reopened an existing digest directory" >&2
    exit 1
  fi
  first=$(python3 -c 'print(open("'"$release_root"'/current").read().strip())')

  echo 'more' >> docs/NOTE.md
  git add docs/NOTE.md
  git commit -qm 'docs: change one file so the complete tree digest moves'
  scripts/desktop_release.py local-dev-package --release-root "$release_root"
  second=$(python3 -c 'print(open("'"$release_root"'/current").read().strip())')
  if [[ "$first" == "$second" ]]; then
    echo "complete source-tree digest ignored a tracked file change" >&2
    exit 1
  fi
  scripts/desktop_release.py local-dev-verify --release-root "$release_root"

  if scripts/desktop_release.py local-dev-rollback --release-root "$release_root" \
      --target-digest sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff >/dev/null 2>&1; then
    echo "rollback trusted a digest that is not a published tree" >&2
    exit 1
  fi
  scripts/desktop_release.py local-dev-rollback --release-root "$release_root" --target-digest "$first"
  current=$(python3 -c 'print(open("'"$release_root"'/current").read().strip())')
  if [[ "$current" != "$first" ]]; then
    echo "rollback did not activate the authenticated target tree" >&2
    exit 1
  fi
  git checkout -q HEAD~1
  scripts/desktop_release.py local-dev-verify --release-root "$release_root"

  if scripts/desktop_release.py local-dev-admit-app --release-root "$release_root" \
      --app-hash sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb \
      --boolean-true >/dev/null 2>&1; then
    echo "boolean admit_macos_app_artifact(true) was treated as proof" >&2
    exit 1
  fi
  if scripts/desktop_release.py local-dev-admit-app --release-root "$release_root" \
      --app-hash sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb \
      --codesign-identity "Developer ID Application: Example (TEAMID1)" >/dev/null 2>&1; then
    echo "caller-supplied codesign identity was treated as evidence" >&2
    exit 1
  fi
  if scripts/desktop_release.py local-dev-admit-app --release-root "$release_root" \
      --app-hash sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb >/dev/null 2>&1; then
    echo "unsigned admit wrote live/" >&2
    exit 1
  fi

  python3 - <<PY
import importlib.util, json, os, pathlib, sys, tempfile
root = pathlib.Path("$release_root")
current = (root / "current").read_text().strip()
base = root / "releases" / current.replace(":", "-")
manifest = json.loads((base / "manifest.json").read_text())
evidence = json.loads((base / "candidate" / "evidence" / "macos-app.json").read_text())
assert manifest["artifacts"]["macos_app"] is None
assert manifest["leftovers"][0]["id"] == "mac-packaged-app-build"
assert manifest["leftovers"][0]["status"] == "needed"
assert any(item["id"] == "approved-macos-signing-pin" and item["status"] == "needed" for item in manifest["leftovers"])
assert manifest["buzz_transport"] == "optional-to-transport"
assert manifest["desktop_requires_relay"] is True
assert manifest["transport_requires_desktop"] is False
assert manifest["relay_ws_url"] == "ws://localhost:3300"
assert manifest["keyring_service"] == "buzz-desktop"
assert manifest["owner_pin"]["status"] == "exact+digest"
assert manifest["owner_pin"]["admission"] == "pinned"
assert manifest["owner_pin"]["owner_pubkey"] == "ea840b3e14aceac2b09619de28aedda628e79fcb120dea462ed3ccc512875971"
assert manifest["owner_pin"]["owner_pubkey_sha256"] == "sha256:af3cd8c1007e504b9d0385c0090395f2a4fecef56e34fd91e66301093583637e"
assert "nsec" not in json.dumps(manifest)
assert not (base / "live").exists()
assert evidence["signed"] is False
assert evidence["notarized"] is False
assert evidence["sha256"] is None or evidence["sha256"].startswith("sha256:")

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop_release", "scripts/desktop_release.py")
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
assert mod.pin_is_compiled(manifest["owner_pin"])
profile_path = pathlib.Path(".release/local-dev-production.json")
original = profile_path.read_bytes()
unpinned = json.loads(original)
unpinned["owner_pubkey"] = None
unpinned["owner_pubkey_sha256"] = None
profile_path.write_text(json.dumps(unpinned, indent=2) + "\n")
assert not mod.pin_is_compiled(manifest["owner_pin"]), "forged exact pin must deny against unpinned compiled JSON"
try:
    mod.require_manifest_pin_matches_compiled(manifest["owner_pin"])
    raise SystemExit("forged exact manifest pin was accepted against unpinned compiled JSON")
except SystemExit as exc:
    text = str(exc)
    if "forged" not in text and "unpinned" not in text and "does not equal" not in text:
        raise
profile_path.write_bytes(original)
assert mod.pin_is_compiled(manifest["owner_pin"])

left = pathlib.Path(tempfile.mkdtemp())
right = pathlib.Path(tempfile.mkdtemp())
(left / "payload").write_text("same")
(right / "payload").write_text("same")
(left / "alias").symlink_to("payload")
os.chmod(right / "payload", 0o600)
assert mod.sha256_tree(left) != mod.sha256_tree(right)
clone = pathlib.Path(tempfile.mkdtemp())
(clone / "payload").write_text("same")
(clone / "alias").symlink_to("payload")
os.chmod(clone / "payload", (left / "payload").stat().st_mode)
assert mod.sha256_tree(left) == mod.sha256_tree(clone)
PY

  fake_app=$(mktemp -d)/Buzz.app
  mkdir -p "$fake_app/Contents/MacOS"
  echo 'not a signed mac app' > "$fake_app/Contents/MacOS/Buzz"
  if scripts/desktop_release.py local-dev-admit-app --release-root "$release_root" \
      --app "$fake_app" >/dev/null 2>&1; then
    echo "unsigned .app path wrote live/" >&2
    exit 1
  fi
  python3 - <<PY
import json, pathlib, sys
root = pathlib.Path("$release_root")
current = (root / "current").read_text().strip()
base = root / "releases" / current.replace(":", "-")
evidence = json.loads((base / "candidate" / "evidence" / "macos-app.json").read_text())
assert evidence["signed"] is False
assert evidence["notarized"] is False
assert evidence["stapled"] is False
assert evidence["codesign_verify"] is False
assert evidence["gatekeeper"] is False
assert evidence["sha256"].startswith("sha256:")
assert not (base / "live").exists()
reason = evidence.get("reason") or ""
assert reason, "unsigned observation must record a fail-closed reason"
if sys.platform == "darwin":
    assert "do not fake macOS tools" not in reason
PY

  unrelated=$(mktemp -d)/Unrelated.app
  mkdir -p "$unrelated/Contents/MacOS" "$unrelated/Contents/Resources/.release"
  printf '%s\n' \
    '<?xml version="1.0" encoding="UTF-8"?>' \
    '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">' \
    '<plist version="1.0"><dict>' \
    '<key>CFBundleIdentifier</key><string>com.apple.Safari</string>' \
    '<key>CFBundleExecutable</key><string>Safari</string>' \
    '<key>CFBundleShortVersionString</key><string>1.0.0</string>' \
    '</dict></plist>' > "$unrelated/Contents/Info.plist"
  echo fake > "$unrelated/Contents/MacOS/Safari"
  echo '{"notarized":true}' > "$unrelated/Contents/Resources/.release/source-receipt.json"
  if scripts/desktop_release.py local-dev-admit-app --release-root "$release_root" \
      --app "$unrelated" >/dev/null 2>&1; then
    echo "unrelated Apple-notarized-looking app wrote live/" >&2
    exit 1
  fi
  python3 - <<PY
import json, pathlib
root = pathlib.Path("$release_root")
current = (root / "current").read_text().strip()
base = root / "releases" / current.replace(":", "-")
evidence = json.loads((base / "candidate" / "evidence" / "macos-app.json").read_text())
assert evidence["signed"] is False
assert evidence["notarized"] is False
assert evidence["bundle_identifier"] == "com.apple.Safari"
assert "xyz.block.buzz.app" in (evidence.get("reason") or "")
assert not (base / "live").exists()
PY

  rm -rf scripts/__pycache__
  still_unsigned=$(mktemp -d)
  git checkout -q HEAD
  echo extra > docs/STILL-UNSIGNED.md
  git add docs/STILL-UNSIGNED.md
  git commit -qm 'docs: compiled owner pin still cannot write unsigned live/'
  scripts/desktop_release.py local-dev-package --release-root "$still_unsigned"
  if scripts/desktop_release.py local-dev-admit-app --release-root "$still_unsigned" \
      --app-hash sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb >/dev/null 2>&1; then
    echo "unsigned admit wrote live/ after compiled owner pin" >&2
    exit 1
  fi
  python3 - <<PY
import json, pathlib
root = pathlib.Path("$still_unsigned")
current = (root / "current").read_text().strip()
base = root / "releases" / current.replace(":", "-")
assert not (base / "live").exists()
manifest = json.loads((base / "manifest.json").read_text())
assert manifest["owner_pin"]["status"] == "exact+digest"
PY
  rm -rf "$still_unsigned"
)

echo "desktop release candidate contract passed"
