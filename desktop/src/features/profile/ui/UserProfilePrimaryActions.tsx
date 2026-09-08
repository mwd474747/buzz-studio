import {
  MessageSquare,
  Pencil,
  Play,
  RefreshCw,
  Sparkles,
  Square,
  UserMinus,
  UserPlus,
} from "lucide-react";
import { toast } from "sonner";

import type {
  useFollowMutation,
  useUnfollowMutation,
} from "@/features/profile/hooks";
import { ProfileQuickAction } from "@/features/profile/ui/ProfileQuickAction";
import { useFeatureEnabled } from "@/shared/features";

// ── Primary actions ──────────────────────────────────────────────────────────

export function ProfilePrimaryActions({
  agentActionDisabled,
  agentActionLabel,
  agentActionLive,
  canEditAgent,
  followMutation,
  isFollowing,
  messagePending,
  onAgentPrimaryAction,
  onAgentRestart,
  onCreateCard,
  onEditAgent,
  onMessage,
  pubkey,
  unfollowMutation,
}: {
  agentActionDisabled?: boolean;
  agentActionLabel?: string;
  agentActionLive?: boolean;
  canEditAgent: boolean;
  followMutation: ReturnType<typeof useFollowMutation>;
  isFollowing: boolean;
  messagePending?: boolean;
  onAgentPrimaryAction?: () => void;
  onAgentRestart?: () => void;
  onCreateCard?: () => void;
  onEditAgent: () => void;
  onMessage?: () => void;
  pubkey: string;
  unfollowMutation: ReturnType<typeof useUnfollowMutation>;
}) {
  const showFollowAction = useFeatureEnabled("pulse");
  const followToggleMutation = isFollowing ? unfollowMutation : followMutation;

  const handleFollowClick = () => {
    followToggleMutation.mutate(pubkey, {
      onError: (error) =>
        toast.error(
          `${isFollowing ? "Unfollow" : "Follow"} failed: ${error.message}`,
        ),
    });
  };

  return (
    <div className="flex items-center justify-center gap-8">
      {showFollowAction ? (
        <ProfileQuickAction
          active={isFollowing}
          disabled={followToggleMutation.isPending}
          icon={isFollowing ? UserMinus : UserPlus}
          label={isFollowing ? "Unfollow" : "Follow"}
          onClick={handleFollowClick}
        />
      ) : null}
      {onMessage ? (
        <ProfileQuickAction
          disabled={messagePending}
          icon={MessageSquare}
          isLoading={messagePending}
          label="Message"
          onClick={onMessage}
          testId="user-profile-message"
        />
      ) : null}
      {canEditAgent ? (
        <ProfileQuickAction
          icon={Pencil}
          label="Edit"
          onClick={onEditAgent}
          testId="user-profile-edit-agent"
        />
      ) : null}
      {onAgentPrimaryAction && agentActionLabel ? (
        <ProfileQuickAction
          active={agentActionLive}
          disabled={agentActionDisabled}
          icon={agentActionLive ? Square : Play}
          label={agentActionLabel}
          onClick={onAgentPrimaryAction}
          testId="user-profile-agent-primary-action"
        />
      ) : null}
      {onAgentRestart ? (
        <ProfileQuickAction
          disabled={agentActionDisabled}
          icon={RefreshCw}
          label="Restart"
          onClick={onAgentRestart}
          testId="user-profile-agent-restart"
        />
      ) : null}
      {onCreateCard ? (
        <ProfileQuickAction
          icon={Sparkles}
          label="Create card"
          onClick={onCreateCard}
          testId="user-profile-create-card"
        />
      ) : null}
    </div>
  );
}

export function ProfilePersonaPrimaryActions({
  canEditAgent,
  disabled,
  onCreateCard,
  onEditAgent,
  onStartAgent,
}: {
  canEditAgent: boolean;
  disabled: boolean;
  onCreateCard?: () => void;
  onEditAgent: () => void;
  onStartAgent: () => void;
}) {
  return (
    <div className="flex items-center justify-center gap-8">
      <ProfileQuickAction
        disabled={disabled}
        icon={Play}
        label="Start agent"
        onClick={onStartAgent}
        testId="user-profile-start-agent"
      />
      {canEditAgent ? (
        <ProfileQuickAction
          disabled={disabled}
          icon={Pencil}
          label="Edit"
          onClick={onEditAgent}
          testId="user-profile-edit-agent"
        />
      ) : null}
      {onCreateCard ? (
        <ProfileQuickAction
          disabled={disabled}
          icon={Sparkles}
          label="Create card"
          onClick={onCreateCard}
          testId="user-profile-create-card"
        />
      ) : null}
    </div>
  );
}
