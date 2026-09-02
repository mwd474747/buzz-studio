import { useQueries, useQuery } from "@tanstack/react-query";

import { fetchCommunityIcon } from "@/shared/api/communityProfile";

import type { Community } from "./types";
import {
  loadCachedCommunityIcon,
  saveCachedCommunityIcon,
} from "./communityIconCache";
import { useLocalOwnerPolicy } from "@/features/onboarding/useLocalOwnerPolicy";

export const communityIconQueryKey = (relayUrl: string) =>
  ["communityIcon", relayUrl] as const;

const ICON_STALE_MS = 5 * 60_000;

async function fetchIconForCommunity(
  community: Community,
): Promise<string | null> {
  const icon = await fetchCommunityIcon(community.relayUrl);
  saveCachedCommunityIcon(community.relayUrl, icon);
  return icon;
}

function iconQueryOptions(community: Community) {
  return {
    queryKey: communityIconQueryKey(community.relayUrl),
    queryFn: () => fetchIconForCommunity(community),
    // Cached icon renders immediately; the fetch still runs and replaces it.
    placeholderData: loadCachedCommunityIcon(community.relayUrl),
    staleTime: ICON_STALE_MS,
    retry: 1,
  };
}

/**
 * Community icons for the rail, keyed by community id. Each icon is read
 * from its relay's NIP-11 document over plain HTTP — active and inactive
 * communities alike. Falls back to the localStorage cache (then null →
 * initials) when a relay is unreachable.
 */
export function useCommunityIcons(
  communities: Community[],
): Record<string, string | null> {
  const localOwnerPolicy = useLocalOwnerPolicy();
  const results = useQueries({
    queries:
      localOwnerPolicy === "inactive"
        ? communities.map((community) => iconQueryOptions(community))
        : [],
  });

  const icons: Record<string, string | null> = {};
  communities.forEach((community, index) => {
    icons[community.id] =
      localOwnerPolicy === "inactive"
        ? (results[index]?.data ?? loadCachedCommunityIcon(community.relayUrl))
        : null;
  });
  return icons;
}

/** Icon of the ACTIVE community, for settings preview. */
export function useActiveCommunityIcon(relayUrl: string | undefined) {
  const localOwnerPolicy = useLocalOwnerPolicy();
  return useQuery({
    queryKey: communityIconQueryKey(relayUrl ?? ""),
    queryFn: async () => {
      const icon = await fetchCommunityIcon(relayUrl ?? "");
      if (relayUrl) saveCachedCommunityIcon(relayUrl, icon);
      return icon;
    },
    enabled: relayUrl !== undefined && localOwnerPolicy === "inactive",
    staleTime: ICON_STALE_MS,
  });
}
