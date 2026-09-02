import * as React from "react";

import { getLocalOwnerProfile } from "@/shared/api/tauriLocalOwner";

export type LocalOwnerPolicyStatus =
  | "loading"
  | "inactive"
  | "active"
  | "unavailable";

/**
 * Resolve whether this binary carries the compiled local-owner policy.
 *
 * Identity replacement stays unavailable until the native command positively
 * reports that no profile is active. The Rust boundary remains authoritative;
 * this hook keeps recovery copy and controls from advertising an operation the
 * pinned build will reject.
 */
export function useLocalOwnerPolicy(): LocalOwnerPolicyStatus {
  const [status, setStatus] = React.useState<LocalOwnerPolicyStatus>("loading");

  React.useEffect(() => {
    let cancelled = false;

    void getLocalOwnerProfile().then(
      (profile) => {
        if (!cancelled) {
          setStatus(profile === null ? "inactive" : "active");
        }
      },
      () => {
        if (!cancelled) {
          setStatus("unavailable");
        }
      },
    );

    return () => {
      cancelled = true;
    };
  }, []);

  return status;
}
