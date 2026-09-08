import { useLegacyAgentCockpitEffects } from "@/features/agents/useLegacyAgentCockpitEffects";

type OrdinaryAgentCockpitEffectsProps = {
  communities: readonly { relayUrl: string }[];
  deferredOwnerPubkey?: string;
  enabled: boolean;
  ownerPubkey?: string;
  relayUrl?: string;
};

/** Ordinary-product effects that do not belong in the local-owner artifact. */
export function OrdinaryAgentCockpitEffects(
  props: OrdinaryAgentCockpitEffectsProps,
) {
  useLegacyAgentCockpitEffects(props);
  return null;
}
