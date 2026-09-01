#!/usr/bin/env bash
set -euo pipefail

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
  scripts/desktop_release.py local-dev-package --release-root "$release_root" --owner-pubkey "$owner"
  scripts/desktop_release.py local-dev-verify --release-root "$release_root"
  if scripts/desktop_release.py local-dev-package --release-root "$release_root" --owner-pubkey "$owner" >/dev/null 2>&1; then
    echo "packager reopened an existing digest directory" >&2
    exit 1
  fi
  first=$(python3 -c 'print(open("'"$release_root"'/current").read().strip())')

  echo 'more' >> docs/NOTE.md
  git add docs/NOTE.md
  git commit -qm 'docs: change one file so the complete tree digest moves'
  scripts/desktop_release.py local-dev-package --release-root "$release_root" --owner-pubkey "$owner"
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
  scripts/desktop_release.py local-dev-admit-app --release-root "$release_root" \
      --app-hash sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
  if scripts/desktop_release.py local-dev-admit-app --release-root "$release_root" \
      --app-hash sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc >/dev/null 2>&1; then
    echo "live publication overwrote an existing live digest" >&2
    exit 1
  fi

  python3 - <<PY
import json, pathlib
root = pathlib.Path("$release_root")
current = (root / "current").read_text().strip()
base = root / "releases" / current.replace(":", "-")
manifest = json.loads((base / "manifest.json").read_text())
live = json.loads((base / "live" / "manifest.json").read_text())
assert manifest["artifacts"]["macos_app"] is None
assert manifest["leftovers"][0]["id"] == "mac-packaged-app-build"
assert manifest["leftovers"][0]["status"] == "needed"
assert manifest["buzz_transport"] == "optional-to-transport"
assert manifest["desktop_requires_relay"] is True
assert manifest["transport_requires_desktop"] is False
assert manifest["relay_ws_url"] == "ws://localhost:3300"
assert manifest["keyring_service"] == "buzz-desktop"
assert manifest["owner_pin"]["status"] == "exact"
assert "nsec" not in json.dumps(manifest)
assert live["leftovers"][0]["status"] == "needed"
assert live["artifacts"]["macos_app"]["sha256"].startswith("sha256:")
assert live["artifacts"]["macos_app"]["signed"] is False
assert live["artifacts"]["macos_app"]["notarized"] is False
PY

  unpinned=$(mktemp -d)
  git checkout -q HEAD
  echo extra > docs/UNPINNED.md
  git add docs/UNPINNED.md
  git commit -qm 'docs: unpinned source package has no invented owner key'
  scripts/desktop_release.py local-dev-package --release-root "$unpinned"
  if scripts/desktop_release.py local-dev-admit-app --release-root "$unpinned" \
      --app-hash sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb >/dev/null 2>&1; then
    echo "live admit succeeded without an owner public-key pin" >&2
    exit 1
  fi
  rm -rf "$unpinned"
)

echo "desktop release candidate contract passed"
