#!/bin/bash -p
set -euo pipefail

trusted_system_path=/usr/bin:/bin:/usr/sbin:/sbin
PATH=$trusted_system_path
export PATH
script_source=${BASH_SOURCE[0]}
if [[ $script_source != /* ]]; then
  script_source=$(builtin pwd -P)/$script_source
fi
if [[ -L $script_source || ! -f $script_source ]]; then
  echo "local-owner build wrapper must be invoked as a regular, non-symlink file" >&2
  exit 1
fi
script_dir=$(builtin cd -- "${script_source%/*}" && builtin pwd -P)
script_path="$script_dir/${script_source##*/}"
repo_root=$(builtin cd -- "$script_dir/.." && builtin pwd -P)
if [[ $script_path != "$repo_root/scripts/build-local-owner-macos.sh" ]]; then
  echo "local-owner build wrapper has an unexpected repository location" >&2
  exit 1
fi
builtin cd -- "$repo_root"

contract_check=false
if [[ ${1:-} == "--contract-check" && $# -eq 1 ]]; then
  contract_check=true
elif (( $# != 0 )); then
  echo "usage: $0 [--contract-check]" >&2
  exit 2
fi

reject_build_environment() {
  local name
  local -a forbidden_environment=()
  while IFS= read -r name; do
    case "$name" in
      BUZZ_* | VITE_* | TAURI_* | NOSTR_PRIVATE_KEY | NOSTR_SECRET_KEY | NSEC | \
        APPLE_CERTIFICATE | APPLE_CERTIFICATE_PASSWORD | APPLE_SIGNING_IDENTITY | \
        APPLE_ID | APPLE_PASSWORD | APPLE_TEAM_ID | AC_API_KEY | AC_API_KEY_ID | \
        AC_API_ISSUER_ID | NODE_OPTIONS | NODE_PATH | RUSTC | RUSTC_* | RUSTFLAGS | \
        RUSTDOC | RUSTDOCFLAGS | RUSTUP_* | CARGO_* | npm_config_* | NPM_CONFIG_* | \
        PNPM_* | GIT_* | CC | CXX | CPP | AR | LD | NM | OBJC | OBJCXX | RANLIB | \
        STRIP | CFLAGS | CXXFLAGS | CPPFLAGS | LDFLAGS | ARFLAGS | DYLD_* | LD_* | \
        BASH_ENV | ENV | BASHOPTS | SHELLOPTS | CDPATH | GLOBIGNORE | HERMIT_* | XDG_* | \
        MACOSX_DEPLOYMENT_TARGET | SDKROOT | DEVELOPER_DIR | PKG_CONFIG_* | \
        CMAKE_* | MAKEFLAGS | MFLAGS | SCCACHE_* | CCACHE_*)
        forbidden_environment+=("$name")
        ;;
    esac
  done < <(compgen -e)
  if (( ${#forbidden_environment[@]} != 0 )); then
    printf 'local-owner builds reject ambient build/signing variables: %s\n' \
      "${forbidden_environment[*]}" >&2
    exit 1
  fi
}

assert_interaction_only_info_plist() {
  local checked_plist=$1
  local forbidden_key
  if ! /usr/bin/plutil -lint "$checked_plist" >/dev/null; then
    echo "local-owner Info.plist is not a valid property list: $checked_plist" >&2
    exit 1
  fi
  for forbidden_key in \
    NSMicrophoneUsageDescription \
    NSCameraUsageDescription \
    NSLocalNetworkUsageDescription \
    CFBundleURLTypes; do
    if /usr/libexec/PlistBuddy -c "Print :$forbidden_key" \
      "$checked_plist" >/dev/null 2>&1; then
      echo "local-owner Info.plist contains forbidden metadata: $forbidden_key" >&2
      exit 1
    fi
  done
  if /usr/bin/plutil -convert xml1 -o - "$checked_plist" \
    | /usr/bin/grep -Eiq \
      '(microphone|camera|local[[:space:]-]+network|share[[:space:]-]+compute)'; then
    echo "local-owner Info.plist contains a legacy device/network purpose string" >&2
    exit 1
  fi
}

unset GIT_PAGER
reject_build_environment

vite_environment_files=(
  desktop/.env
  desktop/.env.local
  desktop/.env.production
  desktop/.env.production.local
)
for path in "${vite_environment_files[@]}"; do
  if [[ -e "$path" || -L "$path" ]]; then
    echo "local-owner builds reject Vite environment file: $path" >&2
    exit 1
  fi
done

profile_path="$repo_root/.release/local-owner-profile.json"
ratification_path="$repo_root/.release/local-owner-ratification.json"
overlay_path="$repo_root/desktop/src-tauri/tauri.local-owner.conf.json"
entitlements_path="$repo_root/desktop/src-tauri/Entitlements.local-owner.plist"
info_plist_path="$repo_root/desktop/src-tauri/Info.local-owner.plist"
capability_path="$repo_root/desktop/src-tauri/capabilities/local-owner.json"
governed_sources=(
  "$profile_path"
  "$ratification_path"
  "$repo_root/.release/LOCAL-OWNER-RATIFICATION.md"
  "$repo_root/.release/LOCAL-OWNER-MACOS-SIGNING-HOLD.md"
  "$overlay_path"
  "$entitlements_path"
  "$info_plist_path"
  "$capability_path"
)
for path in "${governed_sources[@]}"; do
  if [[ ! -f "$path" || -L "$path" ]]; then
    echo "local-owner build input must be a regular, non-symlink file: $path" >&2
    exit 1
  fi
done
if ! /usr/bin/jq -e '
  .schema_version == 1
  and .authority == "mike"
  and .ratified_on == "2026-09-01"
  and .channel == "#local-dev"
  and .owner_pubkey == "ea840b3e14aceac2b09619de28aedda628e79fcb120dea462ed3ccc512875971"
  and .owner_pubkey_sha256 == "sha256:af3cd8c1007e504b9d0385c0090395f2a4fecef56e34fd91e66301093583637e"
  and .authority_receipt_sha256 == "sha256:9ccb24a04428fec6d9638d729bbddf0784c4af0de72c55ef0f3f1c22e9e42517"
' "$ratification_path" >/dev/null; then
  echo "invalid local-owner authority ratification" >&2
  exit 1
fi
if ! /usr/bin/jq -e --slurpfile ratification "$ratification_path" '
  .schema_version == 1
  and .profile == "local-owner"
  and .bundle_identifier == "xyz.block.buzz.app"
  and .owner_pubkey == $ratification[0].owner_pubkey
  and .owner_pubkey_sha256 == $ratification[0].owner_pubkey_sha256
  and .macos_signing.required == true
  and (
    (.macos_signing.team_id == null and .macos_signing.identity == null)
    or (
      (.macos_signing.team_id | type == "string")
      and (.macos_signing.team_id | test("^[A-Z0-9]{10}$"))
      and (.macos_signing.identity | type == "string")
      and (.macos_signing.identity | length > 0)
      and (.macos_signing.identity == (.macos_signing.identity | gsub("^\\s+|\\s+$"; "")))
    )
  )
' "$profile_path" >/dev/null; then
  echo "invalid local-owner profile or macOS signing pins" >&2
  exit 1
fi
if ! /usr/bin/jq -e '
  .identifier == "xyz.block.buzz.app"
  and .app.security.capabilities == ["local-owner"]
  and (.app.security.csp | type == "string")
  and (.app.security.csp | contains("default-src '\''self'\''"))
  and (.app.security.csp | contains("ws://localhost:3300"))
  and (.app.security.csp | contains("buzz-media:"))
  and (.app.security.csp | contains("http://127.0.0.1:") | not)
  and (.app.security.csp | contains("object-src '\''none'\''"))
  and (.app.security.csp | contains("frame-src '\''none'\''"))
  and (.app.security.csp | contains("https://") | not)
  and .plugins["deep-link"].desktop == []
  and .bundle.externalBin == []
  and .bundle.macOS.entitlements == "Entitlements.local-owner.plist"
  and .bundle.macOS.infoPlist == "Info.local-owner.plist"
  and (.bundle.resources | length == 5)
  and .bundle.resources["../../.release/local-owner-profile.json"] == ".release/local-owner-profile.json"
  and .bundle.resources["../../.release/local-owner-ratification.json"] == ".release/local-owner-ratification.json"
  and .bundle.resources["../../.release/LOCAL-OWNER-RATIFICATION.md"] == ".release/LOCAL-OWNER-RATIFICATION.md"
  and .bundle.resources["../../.release/LOCAL-OWNER-MACOS-SIGNING-HOLD.md"] == ".release/LOCAL-OWNER-MACOS-SIGNING-HOLD.md"
  and .bundle.resources["generated/local-owner-source-receipt.json"] == ".release/local-owner-source-receipt.json"
' "$overlay_path" >/dev/null; then
  echo "invalid local-owner Tauri bundle overlay" >&2
  exit 1
fi
if ! /usr/bin/jq -e '
  .identifier == "local-owner"
  and .windows == ["main"]
  and (.permissions | index("websocket:default") != null)
  and (.permissions | index("process:allow-restart") != null)
  and (.permissions | index("notification:allow-register-listener") != null)
  and (.permissions | index("notification:allow-notify") != null)
  and (.permissions | index("core:event:allow-emit-to") == null)
  and ([.permissions[] | select(
    startswith("opener:")
    or startswith("dialog:")
    or startswith("updater:")
    or startswith("global-shortcut:")
    or startswith("window-state:")
    or startswith("core:path:")
  )] | length == 0)
' "$capability_path" >/dev/null; then
  echo "invalid local-owner renderer capability" >&2
  exit 1
fi
if /usr/bin/grep -Fq 'com.apple.security.cs.disable-library-validation' \
  "$entitlements_path"; then
  echo "local-owner entitlements must not disable library validation" >&2
  exit 1
fi
if [[ $(/usr/bin/grep -c '<key>' "$entitlements_path") -ne 0 ]]; then
  echo "local-owner entitlements must not grant device or runtime exceptions" >&2
  exit 1
fi
assert_interaction_only_info_plist "$info_plist_path"

# The contract-only lane still resolves the real repository source identity so
# fake git/jq entries supplied through caller PATH cannot go unnoticed.
/usr/bin/git rev-parse --verify HEAD >/dev/null
/usr/bin/git rev-parse 'HEAD^{tree}' >/dev/null

if [[ $contract_check == true ]]; then
  echo "local-owner release contract is valid"
  exit 0
fi

if [[ $(/usr/bin/uname -s) != "Darwin" ]]; then
  echo "local-owner app builds require a controlled Mac" >&2
  exit 1
fi
if [[ $(/usr/bin/uname -m) != "arm64" ]]; then
  echo "local-owner app builds currently require the ratified Apple silicon toolchain" >&2
  exit 1
fi

trusted_home=${HOME:?local-owner builds require HOME}
login_name=$(/usr/bin/id -un)
trusted_home=$(/usr/bin/dscl . -read "/Users/$login_name" NFSHomeDirectory \
  | /usr/bin/awk 'NR == 1 { print $2 }')
if [[ -z $trusted_home || $HOME != "$trusted_home" ]]; then
  echo "local-owner builds reject an overridden HOME" >&2
  exit 1
fi
cached_hermit="$trusted_home/Library/Caches/hermit/pkg/hermit@stable/hermit"
if [[ ! -f $cached_hermit || -L $cached_hermit || ! -x $cached_hermit ]]; then
  echo "local-owner builds require the installed stable Hermit executable" >&2
  exit 1
fi
build_root=$(/usr/bin/mktemp -d "/private/tmp/buzz-local-owner-build.XXXXXX")
if [[ ! -d $build_root || -L $build_root \
  || $build_root != /private/tmp/buzz-local-owner-build.* ]]; then
  echo "local-owner build root must be a fresh real directory" >&2
  exit 1
fi
/bin/chmod 0700 "$build_root"
cleanup_failed_build() {
  /bin/rm -rf -- "$build_root"
}
trap cleanup_failed_build EXIT

hermit_root="$build_root/hermit"
/bin/mkdir "$hermit_root"
/bin/chmod 0700 "$hermit_root"
verified_hermit="$hermit_root/hermit"
/bin/cp "$cached_hermit" "$verified_hermit"
/bin/chmod 0500 "$verified_hermit"
expected_hermit_sha256=61935bf58de3930bbec196d7c79d2a4d14d9e967670786d0eb433e1c4f567c05
actual_hermit_sha256=$(/usr/bin/shasum -a 256 "$verified_hermit" | /usr/bin/awk '{print $1}')
if [[ $actual_hermit_sha256 != "$expected_hermit_sha256" ]]; then
  echo "local-owner builds require the ratified Hermit executable" >&2
  exit 1
fi
export HERMIT_USER_HOME="$trusted_home"
export HERMIT_STATE_DIR="$hermit_root/state"
export HERMIT_DIST_URL="https://github.com/cashapp/hermit/releases/download/stable"
export HERMIT_CHANNEL=stable
export HERMIT_EXE="$verified_hermit"
build_home="$build_root/home"
/bin/mkdir -p \
  "$build_home" \
  "$build_root/tmp" \
  "$build_root/xdg-cache" \
  "$build_root/xdg-config" \
  "$build_root/xdg-data" \
  "$build_root/xdg-state" \
  "$build_root/rustup-home"
/bin/chmod 0700 \
  "$build_home" \
  "$build_root/tmp" \
  "$build_root/xdg-cache" \
  "$build_root/xdg-config" \
  "$build_root/xdg-data" \
  "$build_root/xdg-state" \
  "$build_root/rustup-home"
export HOME="$build_home"
export TMPDIR="$build_root/tmp"
export XDG_CACHE_HOME="$build_root/xdg-cache"
export XDG_CONFIG_HOME="$build_root/xdg-config"
export XDG_DATA_HOME="$build_root/xdg-data"
export XDG_STATE_HOME="$build_root/xdg-state"
export RUSTUP_HOME="$build_root/rustup-home"
export CARGO_HOME="$build_root/cargo-home"
export CARGO_TARGET_DIR="$build_root/target"

hermit_exec() {
  "$verified_hermit" --level=fatal exec "$build_repo_root" -- "$@"
}

if ! /usr/bin/plutil -lint "$entitlements_path" >/dev/null; then
  echo "local-owner entitlements are not a valid property list" >&2
  exit 1
fi
if /usr/bin/plutil -extract com.apple.security.cs.disable-library-validation raw -o - \
  "$entitlements_path" >/dev/null 2>&1; then
  echo "local-owner entitlements disable library validation" >&2
  exit 1
fi

if [[ -n $(/usr/bin/git status --porcelain --untracked-files=all) ]]; then
  echo "local-owner app builds require a clean worktree" >&2
  exit 1
fi

expected_source_commit=$(/usr/bin/git rev-parse HEAD)
expected_source_tree=$(/usr/bin/git rev-parse 'HEAD^{tree}')

source_bundle="$build_root/source.bundle"
build_repo_root="$build_root/source"
/bin/mkdir "$build_root/git-home" "$build_root/git-config"
isolated_git() {
  HOME="$build_root/git-home" \
    XDG_CONFIG_HOME="$build_root/git-config" \
    GIT_CONFIG_NOSYSTEM=1 \
    /usr/bin/git "$@"
}
isolated_git -C "$repo_root" bundle create "$source_bundle" HEAD
isolated_git clone --no-checkout "$source_bundle" "$build_repo_root"
isolated_git -C "$build_repo_root" checkout --detach "$expected_source_commit"
if [[ $(isolated_git -C "$build_repo_root" rev-parse HEAD) != "$expected_source_commit" ]] \
  || [[ $(isolated_git -C "$build_repo_root" rev-parse 'HEAD^{tree}') != "$expected_source_tree" ]] \
  || [[ -n $(isolated_git -C "$build_repo_root" status --porcelain --untracked-files=all) ]]; then
  echo "local-owner isolated source does not match the approved Git tree" >&2
  exit 1
fi

profile_path="$build_repo_root/.release/local-owner-profile.json"
ratification_path="$build_repo_root/.release/local-owner-ratification.json"
overlay_path="$build_repo_root/desktop/src-tauri/tauri.local-owner.conf.json"
entitlements_path="$build_repo_root/desktop/src-tauri/Entitlements.local-owner.plist"
info_plist_path="$build_repo_root/desktop/src-tauri/Info.local-owner.plist"
capability_path="$build_repo_root/desktop/src-tauri/capabilities/local-owner.json"
vite_environment_files=(
  "$build_repo_root/desktop/.env"
  "$build_repo_root/desktop/.env.local"
  "$build_repo_root/desktop/.env.production"
  "$build_repo_root/desktop/.env.production.local"
)
governed_sources=(
  "$profile_path"
  "$ratification_path"
  "$build_repo_root/.release/LOCAL-OWNER-RATIFICATION.md"
  "$build_repo_root/.release/LOCAL-OWNER-MACOS-SIGNING-HOLD.md"
  "$overlay_path"
  "$entitlements_path"
  "$info_plist_path"
  "$capability_path"
)
for path in "${governed_sources[@]}"; do
  if [[ ! -f "$path" || -L "$path" ]]; then
    echo "isolated local-owner build input must be a regular file: $path" >&2
    exit 1
  fi
done

builtin cd -- "$build_repo_root"
hermit_exec pnpm install --frozen-lockfile --store-dir "$build_root/pnpm-store"

for path in "${vite_environment_files[@]}"; do
  if [[ -e "$path" || -L "$path" ]]; then
    echo "dependency installation created forbidden Vite environment file: $path" >&2
    exit 1
  fi
done
if [[ $(isolated_git -C "$build_repo_root" rev-parse HEAD) != "$expected_source_commit" ]] \
  || [[ $(isolated_git -C "$build_repo_root" rev-parse 'HEAD^{tree}') != "$expected_source_tree" ]] \
  || [[ -n $(isolated_git -C "$build_repo_root" status --porcelain --untracked-files=all) ]]; then
  echo "dependency installation changed the local-owner source; refusing provenance" >&2
  exit 1
fi

source_commit=$expected_source_commit
source_tree=$expected_source_tree
profile_sha256="sha256:$(/usr/bin/shasum -a 256 "$profile_path" | /usr/bin/awk '{print $1}')"
receipt_dir="$build_repo_root/desktop/src-tauri/generated"
receipt_path="$receipt_dir/local-owner-source-receipt.json"

if [[ -e "$receipt_dir" || -L "$receipt_dir" ]] \
  && [[ ! -d "$receipt_dir" || -L "$receipt_dir" ]]; then
  echo "local-owner receipt directory must not be redirected" >&2
  exit 1
fi
/bin/mkdir -p "$receipt_dir"
if [[ ! -d "$receipt_dir" || -L "$receipt_dir" ]]; then
  echo "local-owner receipt directory must be a real directory" >&2
  exit 1
fi
receipt_tmp=$(/usr/bin/mktemp "$receipt_dir/.local-owner-source-receipt.XXXXXX")
/usr/bin/jq -n \
  --arg source_commit "$source_commit" \
  --arg source_tree "$source_tree" \
  --arg profile_sha256 "$profile_sha256" \
  '{
    schema_version: 1,
    profile: "local-owner",
    source_commit: $source_commit,
    source_tree: $source_tree,
    profile_sha256: $profile_sha256,
    builder_class: "buzz-local-owner-tauri-wrapper",
    artifact_stage: "unsigned-before-apple-signing",
    source_dirty: false
  }' >"$receipt_tmp"
/bin/chmod 0644 "$receipt_tmp"
/bin/mv "$receipt_tmp" "$receipt_path"

export BUZZ_DESKTOP_SOURCE_COMMIT="$source_commit"
export BUZZ_DESKTOP_SOURCE_TREE="$source_tree"
builtin cd -- "$build_repo_root/desktop"
hermit_exec pnpm tauri build \
  --verbose \
  --no-sign \
  --features local-owner-profile \
  --config src-tauri/tauri.local-owner.conf.json \
  --bundles app

app_path="$CARGO_TARGET_DIR/release/bundle/macos/Buzz.app"
if [[ ! -d "$app_path" || -L "$app_path" ]]; then
  echo "local-owner build did not produce $app_path" >&2
  exit 1
fi
if /usr/bin/find "$app_path" -type l -print -quit | /usr/bin/grep -q .; then
  echo "local-owner app must not contain symlinks" >&2
  exit 1
fi

assert_interaction_only_info_plist "$app_path/Contents/Info.plist"

bundle_identifier=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' \
  "$app_path/Contents/Info.plist")
if [[ $bundle_identifier != "xyz.block.buzz.app" ]]; then
  echo "local-owner app has unexpected bundle identifier: $bundle_identifier" >&2
  exit 1
fi
executable_name=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' \
  "$app_path/Contents/Info.plist")
if [[ -z $executable_name || $executable_name == */* \
  || ! -f "$app_path/Contents/MacOS/$executable_name" \
  || -L "$app_path/Contents/MacOS/$executable_name" \
  || ! -x "$app_path/Contents/MacOS/$executable_name" ]]; then
  echo "local-owner app is missing its declared main executable" >&2
  exit 1
fi
main_executable="$app_path/Contents/MacOS/$executable_name"
if /usr/bin/find "$app_path/Contents" -type f -perm -111 \
  ! -path "$main_executable" -print -quit | /usr/bin/grep -q .; then
  echo "local-owner app contains unexpected executable code" >&2
  exit 1
fi

embedded_entitlements=$(/usr/bin/mktemp "${TMPDIR:-/tmp}/buzz-local-owner-entitlements.XXXXXX")
if /usr/bin/codesign -d --entitlements :- "$main_executable" \
  >"$embedded_entitlements" 2>/dev/null; then
  if [[ -s $embedded_entitlements ]] \
    && { ! /usr/bin/plutil -lint "$embedded_entitlements" >/dev/null \
      || /usr/bin/plutil -extract com.apple.security.cs.disable-library-validation raw -o - \
        "$embedded_entitlements" >/dev/null 2>&1; }; then
    echo "local-owner executable contains unapproved library-validation entitlement" >&2
    /bin/rm -f -- "$embedded_entitlements"
    exit 1
  fi
fi
/bin/rm -f -- "$embedded_entitlements"

resource_sources=(
  "$profile_path"
  "$ratification_path"
  "$build_repo_root/.release/LOCAL-OWNER-RATIFICATION.md"
  "$build_repo_root/.release/LOCAL-OWNER-MACOS-SIGNING-HOLD.md"
  "$receipt_path"
)
resource_destinations=(
  "$app_path/Contents/Resources/.release/local-owner-profile.json"
  "$app_path/Contents/Resources/.release/local-owner-ratification.json"
  "$app_path/Contents/Resources/.release/LOCAL-OWNER-RATIFICATION.md"
  "$app_path/Contents/Resources/.release/LOCAL-OWNER-MACOS-SIGNING-HOLD.md"
  "$app_path/Contents/Resources/.release/local-owner-source-receipt.json"
)
for index in "${!resource_sources[@]}"; do
  if [[ ! -f "${resource_destinations[$index]}" \
    || -L "${resource_destinations[$index]}" ]] \
    || ! /usr/bin/cmp -s "${resource_sources[$index]}" "${resource_destinations[$index]}"; then
    echo "local-owner app resource is missing or changed: ${resource_destinations[$index]}" >&2
    exit 1
  fi
done

if /usr/bin/find "$app_path/Contents/MacOS" -maxdepth 1 \( -type f -o -type l \) \
  \( -name buzz-acp -o -name 'buzz-acp-*' \
  -o -name buzz-agent -o -name 'buzz-agent-*' \
  -o -name buzz-dev-mcp -o -name 'buzz-dev-mcp-*' \
  -o -name git-credential-nostr -o -name 'git-credential-nostr-*' \
  -o -name buzz \) -print -quit | /usr/bin/grep -q .; then
  echo "local-owner app unexpectedly contains a legacy agent/CLI sidecar" >&2
  exit 1
fi

builtin cd -- "$build_repo_root"
if [[ $(isolated_git -C "$build_repo_root" rev-parse HEAD) != "$source_commit" ]] \
  || [[ $(isolated_git -C "$build_repo_root" rev-parse 'HEAD^{tree}') != "$source_tree" ]] \
  || [[ -n $(isolated_git -C "$build_repo_root" status --porcelain --untracked-files=all) ]]; then
  echo "source changed during the local-owner build; rejecting the app" >&2
  exit 1
fi
/bin/rm -rf -- "$build_repo_root" "$source_bundle" "$hermit_root" \
  "$build_root/cargo-home" "$build_root/pnpm-store" \
  "$build_root/git-home" "$build_root/git-config"
trap - EXIT
echo "$app_path"
