import { useArchiveSync } from "@/features/local-archive/archiveSyncManager";
import { useAgentMetricArchiveSeed } from "@/features/local-archive/useAgentMetricArchiveSeed";
import { useObserverArchiveReconciliation } from "@/features/local-archive/useObserverArchiveSeed";

import { useAgentsDataRefresh } from "./lib/useAgentsDataRefresh";
import { useAutoRestartPolicy } from "./lib/useAutoRestartPolicy";
import { usePersonaSync } from "./lib/usePersonaSync";
import { useAgentObserverIngestion } from "./useAgentObserverIngestion";
import { useManagedAgentRuntimeReconciliation } from "./useManagedAgentRuntimeReconciliation";

type Inputs = {
  communities: readonly { relayUrl: string }[];
  deferredOwnerPubkey?: string;
  enabled: boolean;
  ownerPubkey?: string;
  relayUrl?: string;
};

/** Keep the retired agent/telemetry plane out of the local-owner cockpit. */
export function useLegacyAgentCockpitEffects({
  communities,
  deferredOwnerPubkey,
  enabled,
  ownerPubkey,
  relayUrl,
}: Inputs): void {
  const admittedOwner = enabled ? ownerPubkey : undefined;
  useManagedAgentRuntimeReconciliation(communities, enabled);
  usePersonaSync(admittedOwner, relayUrl);
  useAgentsDataRefresh(enabled);
  useAutoRestartPolicy(enabled);
  useAgentObserverIngestion(enabled);
  const observerReady = useObserverArchiveReconciliation(admittedOwner);
  useArchiveSync(observerReady);
  useAgentMetricArchiveSeed(enabled ? deferredOwnerPubkey : undefined);
}
