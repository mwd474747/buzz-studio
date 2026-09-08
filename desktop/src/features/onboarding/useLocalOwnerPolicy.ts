import * as React from "react";

import { getLocalOwnerProfile } from "@/shared/api/tauriLocalOwner";

export type LocalOwnerPolicyStatus =
  | "loading"
  | "inactive"
  | "active"
  | "unavailable";

type ResolvedLocalOwnerPolicy = Exclude<LocalOwnerPolicyStatus, "loading">;

let resolvedPolicy: ResolvedLocalOwnerPolicy | null = null;
let inflight: Promise<ResolvedLocalOwnerPolicy> | null = null;

/** Test-only: drop the process-wide profile cache. */
export function resetLocalOwnerPolicyCache(): void {
  resolvedPolicy = null;
  inflight = null;
}

function resolveLocalOwnerPolicy(): Promise<ResolvedLocalOwnerPolicy> {
  if (resolvedPolicy) {
    return Promise.resolve(resolvedPolicy);
  }
  if (!inflight) {
    inflight = getLocalOwnerProfile().then(
      (profile) => {
        resolvedPolicy = profile === null ? "inactive" : "active";
        return resolvedPolicy;
      },
      () => {
        resolvedPolicy = "unavailable";
        return resolvedPolicy;
      },
    );
  }
  return inflight;
}

/**
 * Resolve whether this binary carries the compiled local-owner policy.
 *
 * Identity replacement stays unavailable until the native command positively
 * reports that no profile is active. The Rust boundary remains authoritative;
 * this hook keeps recovery copy and controls from advertising an operation the
 * pinned build will reject.
 *
 * The compiled profile is process-wide, not community-scoped. Cache the first
 * successful resolution so Settings remounts do not restart as `loading` and
 * rewrite the requested section.
 */
export function useLocalOwnerPolicy(): LocalOwnerPolicyStatus {
  const [status, setStatus] = React.useState<LocalOwnerPolicyStatus>(
    () => resolvedPolicy ?? "loading",
  );

  React.useEffect(() => {
    let cancelled = false;

    void resolveLocalOwnerPolicy().then((next) => {
      if (!cancelled) {
        setStatus(next);
      }
    });

    return () => {
      cancelled = true;
    };
  }, []);

  return status;
}
