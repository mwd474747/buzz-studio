import { invokeTauri } from "@/shared/api/tauri";

export type LocalOwnerProfileSummary = {
  profile: string;
  profile_sha256: string;
  source_commit: string | null;
  source_tree: string | null;
  bundle_identifier: string;
  keyring_service: string;
  relay_ws_url: string;
  owner_pubkey: string;
  owner_pubkey_sha256: string;
  macos_signing_required: boolean;
  macos_signing_configured: boolean;
};

export function getLocalOwnerProfile(): Promise<LocalOwnerProfileSummary | null> {
  return invokeTauri<LocalOwnerProfileSummary | null>(
    "get_local_owner_profile",
  );
}
