import { CommunityMembersSettingsCard } from "@/features/community-members/ui/CommunityMembersSettingsCard";
import { LocalArchiveSettingsCard } from "@/features/local-archive/ui/LocalArchiveSettingsCard";
import { MeshComputeSettingsCard } from "@/features/mesh-compute/ui/MeshComputeSettingsCard";
import { UpdateChecker } from "@/features/settings/UpdateChecker";
import { AgentDefaultsSettingsCard } from "./AgentDefaultsSettingsCard";
import { ChannelTemplatesSettingsCard } from "./ChannelTemplatesSettingsCard";
import { ExperimentalFeaturesCard } from "./ExperimentalFeaturesCard";
import { HarnessesSettingsPanel } from "./HarnessesSettingsPanel";
import { HostedCommunitiesSettingsCard } from "./HostedCommunitiesSettingsCard";
import { MobilePairingCard } from "./MobilePairingCard";
import { ModerationQueueCard } from "./ModerationQueueCard";
import { PreventSleepSettingsCard } from "./PreventSleepSettingsCard";
import type { SettingsPanelProps, SettingsSection } from "./SettingsPanels";
import { VoiceSettingsCard } from "./VoiceSettingsCard";

type OrdinarySettingsSectionProps = {
  props: SettingsPanelProps;
  section: Exclude<
    SettingsSection,
    "appearance" | "custom-emoji" | "notifications" | "profile" | "shortcuts"
  >;
};

/** Settings that are absent from the governed local-owner product graph. */
export function OrdinarySettingsSection({
  props,
  section,
}: OrdinarySettingsSectionProps) {
  switch (section) {
    case "voice":
      return <VoiceSettingsCard />;
    case "experimental":
      return <ExperimentalFeaturesCard />;
    case "agents":
      return (
        <div className="space-y-12">
          <PreventSleepSettingsCard />
          <HarnessesSettingsPanel />
          <AgentDefaultsSettingsCard />
        </div>
      );
    case "channel-templates":
      return <ChannelTemplatesSettingsCard />;
    case "compute":
      return <MeshComputeSettingsCard />;
    case "hosted-communities":
      return <HostedCommunitiesSettingsCard />;
    case "community-members":
      return (
        <CommunityMembersSettingsCard currentPubkey={props.currentPubkey} />
      );
    case "moderation":
      return <ModerationQueueCard />;
    case "local-archive":
      return <LocalArchiveSettingsCard />;
    case "mobile":
      return <MobilePairingCard currentPubkey={props.currentPubkey} />;
    case "updates":
      return <UpdateChecker />;
  }
}
