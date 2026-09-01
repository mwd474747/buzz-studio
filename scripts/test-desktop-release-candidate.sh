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
assert any(item["id"] == "mac-controlled-candidate-producer" and item["status"] == "needed" for item in manifest["leftovers"])
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

  auth_root=$(mktemp -d)
  scripts/desktop_release.py local-dev-package --release-root "$auth_root"
  if scripts/desktop_release.py local-dev-produce-app --release-root "$auth_root" >/dev/null 2>&1; then
    echo "producer manufactured a signed .app on $(uname -s)" >&2
    exit 1
  fi
  caller_app=$(mktemp -d)/Caller.app
  mkdir -p "$caller_app/Contents/MacOS"
  echo caller > "$caller_app/Contents/MacOS/Buzz"
  if scripts/desktop_release.py local-dev-produce-app --release-root "$auth_root" \
      --unsigned-app "$caller_app" >/dev/null 2>&1; then
    echo "producer accepted caller-supplied .app bytes on $(uname -s)" >&2
    exit 1
  fi
  python3 - <<PY
import importlib.util, json, os, pathlib, shutil, sys, tempfile
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop_release", "scripts/desktop_release.py")
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
root = pathlib.Path("$auth_root")
current, dest, manifest = mod.authenticate_source_package(root)
assert current == manifest["content_digest"]
assert dest.parent.resolve() == (root / "releases").resolve()
assert not (dest / "live").exists()
producer = dest / "candidate" / "unsigned" / "producer-leftover.json"
assert producer.is_file(), f"producer leftover missing on {sys.platform}"
leftover = json.loads(producer.read_text())
assert leftover["id"] == "mac-controlled-candidate-producer"
assert leftover["status"] == "needed"
assert leftover.get("stage") == 3
assert leftover.get("attestation_class") == "self-attested-disabled"
assert leftover.get("platform") == sys.platform
assert "hard-disabled" in leftover["reason"]
assert not (dest / "candidate" / "unsigned" / "build-provenance.json").exists()

# Pointer traversal
bad_root = pathlib.Path(tempfile.mkdtemp())
(bad_root / "releases").mkdir()
(bad_root / "current").write_text("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/../etc\n")
try:
    mod.authenticate_source_package(bad_root)
    raise SystemExit("traversal current pointer was accepted")
except SystemExit as exc:
    if "exactly sha256:" not in str(exc):
        raise
outside = pathlib.Path(tempfile.mkdtemp())
(outside / "manifest.json").write_text("{}\n")
hex64 = "b" * 64
link = bad_root / "releases" / f"sha256-{hex64}"
link.symlink_to(outside)
(bad_root / "current").write_text(f"sha256:{hex64}\n")
try:
    mod.authenticate_source_package(bad_root)
    raise SystemExit("symlinked package destination escaped the release root")
except SystemExit as exc:
    if "escaped" not in str(exc) and "direct child" not in str(exc) and "missing" not in str(exc):
        raise

# Tampered base manifest / proof / profile must deny before writes
tamper = pathlib.Path(tempfile.mkdtemp())
shutil.copytree(root / "releases", tamper / "releases")
shutil.copy(root / "current", tamper / "current")
tamper_dest = tamper / "releases" / current.replace(":", "-")
before = list((tamper_dest / "candidate").rglob("*")) if (tamper_dest / "candidate").exists() else []
manifest_path = tamper_dest / "manifest.json"
data = json.loads(manifest_path.read_text())
data["content_digest"] = "sha256:" + ("ee" * 32)
manifest_path.write_text(json.dumps(data, indent=2) + "\n")
try:
    mod.authenticate_source_package(tamper)
    raise SystemExit("tampered manifest digest was accepted")
except SystemExit:
    pass
after = list((tamper_dest / "candidate").rglob("*")) if (tamper_dest / "candidate").exists() else []
assert after == before, "authenticator failure wrote evidence"

# Complete canonical compare: authority-bearing fields
def tamper_field(field, value, label):
    copy = pathlib.Path(tempfile.mkdtemp())
    shutil.copytree(root / "releases", copy / "releases")
    shutil.copy(root / "current", copy / "current")
    path = copy / "releases" / current.replace(":", "-") / "manifest.json"
    body = json.loads(path.read_text())
    body[field] = value
    path.write_text(json.dumps(body, indent=2) + "\n")
    try:
        mod.authenticate_source_package(copy)
        raise SystemExit(f"tampered {label} was accepted")
    except SystemExit as exc:
        text = str(exc)
        if field not in text and "canonical" not in text and "authority-bearing" not in text and "does not match" not in text:
            raise SystemExit(f"tampered {label} failed for the wrong reason: {text}") from exc
    assert not (copy / "releases" / current.replace(":", "-") / "live").exists()

tamper_field("schema_version", 99, "schema_version")
tamper_field("frontend", {"mode": "forged"}, "frontend proof")
tamper_field("state_root", "/tmp/forged-state", "state_root")
tamper_field("log_root", "/tmp/forged-log", "log_root")
tamper_field("transport_requires_desktop", True, "transport_requires_desktop")
tamper_field("lane", "forged-lane", "lane")

extra = pathlib.Path(tempfile.mkdtemp())
shutil.copytree(root / "releases", extra / "releases")
shutil.copy(root / "current", extra / "current")
extra_path = extra / "releases" / current.replace(":", "-") / "manifest.json"
extra_body = json.loads(extra_path.read_text())
extra_body["extra_authority"] = True
extra_path.write_text(json.dumps(extra_body, indent=2) + "\n")
try:
    mod.authenticate_source_package(extra)
    raise SystemExit("extra authority-bearing field was accepted")
except SystemExit as exc:
    if "extra" not in str(exc):
        raise
missing = pathlib.Path(tempfile.mkdtemp())
shutil.copytree(root / "releases", missing / "releases")
shutil.copy(root / "current", missing / "current")
missing_path = missing / "releases" / current.replace(":", "-") / "manifest.json"
missing_body = json.loads(missing_path.read_text())
del missing_body["lane"]
missing_path.write_text(json.dumps(missing_body, indent=2) + "\n")
try:
    mod.authenticate_source_package(missing)
    raise SystemExit("missing authority-bearing field was accepted")
except SystemExit as exc:
    if "missing" not in str(exc) and "lane" not in str(exc):
        raise

tamper2 = pathlib.Path(tempfile.mkdtemp())
shutil.copytree(root / "releases", tamper2 / "releases")
shutil.copy(root / "current", tamper2 / "current")
t2 = tamper2 / "releases" / current.replace(":", "-")
proof = json.loads((t2 / "proofs" / "source-tree.json").read_text())
proof[0]["digest"] = "sha256:" + ("11" * 32)
(t2 / "proofs" / "source-tree.json").write_text(json.dumps(proof) + "\n")
try:
    mod.authenticate_source_package(tamper2)
    raise SystemExit("tampered source-tree proof was accepted")
except SystemExit:
    pass
assert not (t2 / "live").exists()
tamper3 = pathlib.Path(tempfile.mkdtemp())
shutil.copytree(root / "releases", tamper3 / "releases")
shutil.copy(root / "current", tamper3 / "current")
t3 = tamper3 / "releases" / current.replace(":", "-")
(t3 / "profile.json").write_bytes(b'{"tampered":true}\n')
try:
    mod.authenticate_source_package(tamper3)
    raise SystemExit("tampered stored profile was accepted")
except SystemExit:
    pass
assert not (t3 / "candidate" / "evidence").exists() or not list((t3 / "candidate" / "evidence").glob("*"))

# PATH spoofing
try:
    mod._run_macos_tool(["codesign", "--verify", "x"])
    raise SystemExit("PATH-resolved codesign was accepted")
except SystemExit as exc:
    if "trusted absolute path" not in str(exc) and "PATH spoofing" not in str(exc):
        raise
try:
    mod._run_macos_tool(["/tmp/codesign", "--verify", "x"])
    raise SystemExit("untrusted absolute codesign was accepted")
except SystemExit as exc:
    if "trusted absolute path" not in str(exc):
        raise
try:
    mod._run_macos_tool(["/usr/bin/xcrun", "not-stapler", "validate", "x"])
    raise SystemExit("xcrun without stapler was accepted")
except SystemExit as exc:
    if "stapler" not in str(exc):
        raise

# Descendant write escape via symlink at candidate/evidence or candidate/unsigned
escaped = pathlib.Path(tempfile.mkdtemp())
escape_pkg = pathlib.Path(tempfile.mkdtemp())
shutil.copytree(dest, escape_pkg, dirs_exist_ok=True)
candidate_dir = escape_pkg / "candidate"
if candidate_dir.exists() or candidate_dir.is_symlink():
    shutil.rmtree(candidate_dir) if candidate_dir.is_dir() and not candidate_dir.is_symlink() else candidate_dir.unlink()
candidate_dir.mkdir()
(candidate_dir / "evidence").symlink_to(escaped)
try:
    mod.write_package_file(escape_pkg, "candidate/evidence/macos-app.json", "{}\n")
    raise SystemExit("symlink evidence destination was written")
except SystemExit as exc:
    if "symlink" not in str(exc):
        raise
assert not (escaped / "macos-app.json").exists()
unsigned_escape = pathlib.Path(tempfile.mkdtemp())
unsigned_pkg = pathlib.Path(tempfile.mkdtemp())
shutil.copytree(dest, unsigned_pkg, dirs_exist_ok=True)
uc = unsigned_pkg / "candidate"
if uc.exists() or uc.is_symlink():
    shutil.rmtree(uc) if uc.is_dir() and not uc.is_symlink() else uc.unlink()
uc.mkdir()
(uc / "unsigned").symlink_to(unsigned_escape)
try:
    mod.write_package_file(unsigned_pkg, "candidate/unsigned/producer-leftover.json", "{}\n")
    raise SystemExit("symlink unsigned destination was written")
except SystemExit as exc:
    if "symlink" not in str(exc):
        raise
assert not (unsigned_escape / "producer-leftover.json").exists()
try:
    mod.write_package_file(dest, "../escaped.json", "{}\n")
    raise SystemExit("traversal package write was accepted")
except SystemExit as exc:
    if "traversal" not in str(exc) and "relative" not in str(exc):
        raise

# Missing / wrong embedded resources
app = pathlib.Path(tempfile.mkdtemp()) / "Buzz.app"
(app / "Contents/MacOS").mkdir(parents=True)
(app / "Contents/Resources/.release").mkdir(parents=True)
(app / "Contents/Info.plist").write_bytes(b"""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>xyz.block.buzz.app</string>
<key>CFBundleExecutable</key><string>Buzz</string>
<key>CFBundleShortVersionString</key><string>0.0.0-test</string>
</dict></plist>
""")
(app / "Contents/MacOS/Buzz").write_text("unsigned-executable")
evidence = mod.macos_signing_evidence(app, source_manifest=manifest, dest=dest)
assert evidence["signed"] is False
assert "embedded production profile" in (evidence.get("reason") or "")
(app / "Contents/Resources/.release/local-dev-production.json").write_bytes(
    pathlib.Path(".release/local-dev-production.json").read_bytes()
)
(app / "Contents/Resources/.release/source-receipt.json").write_text("{}\n")
evidence = mod.macos_signing_evidence(app, source_manifest=manifest, dest=dest)
assert evidence["signed"] is False
assert "receipt" in (evidence.get("reason") or "")

# Self-attesting embed is hard-disabled
try:
    mod.embed_profile_and_receipt(app, manifest)
    raise SystemExit("self-attesting embed manufactured provenance")
except SystemExit as exc:
    if "hard-disabled" not in str(exc) and "caller-supplied" not in str(exc):
        raise

# Receipt present; self-attested / caller-supplied provenance must refuse live/
exec_digest = mod.sha256_file(app / "Contents/MacOS/Buzz")
receipt = {
    "source_commit": manifest["source_commit"],
    "content_digest": manifest["content_digest"],
    "compiled_profile_sha256": manifest["compiled_profile_sha256"],
    "version": "0.0.0-test",
    "executable_sha256": exec_digest,
    "provenance_sha256": "sha256:" + ("00" * 32),
}
(app / "Contents/Resources/.release/source-receipt.json").write_text(json.dumps(receipt) + "\n")
evidence = mod.macos_signing_evidence(app, source_manifest=manifest, dest=dest)
assert evidence["signed"] is False
assert evidence["provenance_matches"] is False
reason = evidence.get("reason") or ""
assert "self-attested" in reason or "caller-supplied" in reason
mod.write_package_file(
    dest,
    "candidate/unsigned/build-provenance.json",
    json.dumps(
        {
            "attestation_class": "self-attested",
            "executable_sha256": exec_digest,
            "content_digest": manifest["content_digest"],
            "compiled_profile_sha256": manifest["compiled_profile_sha256"],
            "source_commit": manifest["source_commit"],
        },
        indent=2,
    )
    + "\n",
)
evidence = mod.macos_signing_evidence(app, source_manifest=manifest, dest=dest)
assert evidence["signed"] is False
assert evidence["provenance_matches"] is False
assert "self-attested" in (evidence.get("reason") or "") or "caller-supplied" in (evidence.get("reason") or "")
mod.write_package_file(
    dest,
    "candidate/unsigned/build-provenance.json",
    json.dumps(
        {
            "attestation_class": "independent-builder-attestation",
            "builder": "caller-supplied-app",
            "executable_sha256": exec_digest,
            "content_digest": manifest["content_digest"],
            "compiled_profile_sha256": manifest["compiled_profile_sha256"],
            "source_commit": manifest["source_commit"],
        },
        indent=2,
    )
    + "\n",
)
evidence = mod.macos_signing_evidence(app, source_manifest=manifest, dest=dest)
assert evidence["signed"] is False
assert "isolated-mac-lane" in (evidence.get("reason") or "") or "caller-supplied" in (evidence.get("reason") or "")

# Unreadable / unsupported tree entries fail closed
fifo_dir = pathlib.Path(tempfile.mkdtemp())
os.mkfifo(fifo_dir / "pipe")
try:
    mod.sha256_tree(fifo_dir)
    raise SystemExit("fifo tree entry was skipped")
except SystemExit as exc:
    if "unsupported" not in str(exc):
        raise
secret_dir = pathlib.Path(tempfile.mkdtemp())
secret = secret_dir / "secret"
secret.write_text("hidden")
os.chmod(secret, 0)
try:
    if os.geteuid() != 0:
        try:
            mod.sha256_tree(secret_dir)
            raise SystemExit("unreadable file was skipped")
        except SystemExit as exc:
            if "unreadable" not in str(exc):
                raise
finally:
    os.chmod(secret, 0o644)

# SYNTHETIC leftover: injected all-success evidence is not a live proof.
# live admission fails closed without independent builder attestation.
def test_pins(_profile=None):
    return {
        "approved_team_id": "TESTONLYID",
        "approved_codesign_identity": "Test-Only Identity",
        "required": True,
        "filled": True,
    }
mod.compiled_macos_signing_pins = test_pins
live_evidence = {
    "app_path": "/tmp/TestOnly.app",
    "sha256": "sha256:" + ("ab" * 32),
    "signed": True,
    "notarized": True,
    "stapled": True,
    "codesign_verify": True,
    "gatekeeper": True,
    "embedded_profile_matches": True,
    "receipt_matches": True,
    "provenance_matches": True,
    "provenance_attestation_class": "independent-builder-attestation",
    "bundle_identifier": "xyz.block.buzz.app",
    "team_id": "TESTONLYID",
    "codesign_identity": "Test-Only Identity",
    "notarization": "stapler-validate",
    "executable": "/tmp/TestOnly.app/Contents/MacOS/Buzz",
    "executable_sha256": "sha256:" + ("cd" * 32),
    "version": "0.0.0-test",
    "receipt": {"test_only": True},
    "reason": "synthetic leftover: injected all-success evidence",
}
try:
    mod.write_live_if_proven(dest, manifest, live_evidence)
    raise SystemExit("synthetic all-success evidence wrote live/")
except SystemExit as exc:
    text = str(exc)
    if "live/" not in text and "self-attested" not in text and "caller-supplied" not in text and "leftover" not in text:
        raise
assert not (dest / "live").exists(), "synthetic leftover must not write live/"
PY
  # Real leftover producer-to-admission path (unsigned). Does not claim live/ is proven.
  leftover_app=$(mktemp -d)/Buzz.app
  mkdir -p "$leftover_app/Contents/MacOS"
  echo unsigned > "$leftover_app/Contents/MacOS/Buzz"
  if scripts/desktop_release.py local-dev-admit-app --release-root "$auth_root" \
      --app "$leftover_app" >/dev/null 2>&1; then
    echo "producer-to-admission leftover path wrote live/ on $(uname -s)" >&2
    exit 1
  fi
  python3 - <<PY
import json, pathlib
root = pathlib.Path("$auth_root")
current = (root / "current").read_text().strip()
base = root / "releases" / current.replace(":", "-")
assert not (base / "live").exists()
evidence = json.loads((base / "candidate" / "evidence" / "macos-app.json").read_text())
assert evidence["signed"] is False
assert evidence["notarized"] is False
leftover = json.loads((base / "candidate" / "unsigned" / "producer-leftover.json").read_text())
assert leftover["status"] == "needed"
assert leftover.get("stage") == 3
PY
  rm -rf "$auth_root"
)

echo "desktop release candidate contract passed"
