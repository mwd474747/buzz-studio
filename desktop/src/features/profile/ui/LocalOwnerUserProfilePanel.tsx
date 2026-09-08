import { Fingerprint, MessageSquare } from "lucide-react";
import * as React from "react";

import { getPresenceLabel } from "@/features/presence/lib/presence";
import { PresenceDot } from "@/features/presence/ui/PresenceBadge";
import { usePresenceQuery } from "@/features/presence/hooks";
import { useUserProfileQuery } from "@/features/profile/hooks";
import { ProfileAvatar } from "@/features/profile/ui/ProfileAvatar";
import { UserProfilePanelFrame } from "@/features/profile/ui/UserProfilePanelFrame";
import type { UserProfilePanelProps } from "@/features/profile/ui/UserProfilePanelUtils";
import { useProfileDmAction } from "@/features/profile/ui/useProfileDmAction";
import { useUserStatusQuery } from "@/features/user-status/hooks";
import { StatusEmoji } from "@/features/user-status/ui/StatusEmoji";
import { useEscapeKey } from "@/shared/hooks/useEscapeKey";
import { useIsThreadPanelOverlay } from "@/shared/hooks/use-mobile";
import {
  AuxiliaryPanelBody,
  AuxiliaryPanelHeaderGroup,
  AuxiliaryPanelHeaderTitleBlock,
} from "@/shared/layout/AuxiliaryPanel";
import { normalizePubkey, truncatePubkey } from "@/shared/lib/pubkey";
import { cn } from "@/shared/lib/cn";
import { PubKey } from "@/shared/ui/PubKey";
import { Spinner } from "@/shared/ui/spinner";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/shared/ui/tooltip";

/** Human identity and messaging surface for the interaction-only product. */
export function LocalOwnerUserProfilePanel({
  canResetWidth,
  currentPubkey,
  isSinglePanelView = false,
  layout = "standalone",
  onClose,
  onOpenDm,
  onResetWidth,
  onResizeStart,
  pubkey,
  splitPaneClamp = false,
  transparentChrome = false,
  widthPx,
}: UserProfilePanelProps) {
  const isOverlay = useIsThreadPanelOverlay();
  const isSplitLayout = layout === "split";
  const effectivePubkey = pubkey ?? null;
  const normalizedPubkey = normalizePubkey(effectivePubkey ?? "");
  const profileQuery = useUserProfileQuery(effectivePubkey ?? undefined);
  const presenceQuery = usePresenceQuery(
    effectivePubkey ? [effectivePubkey] : [],
  );
  const userStatusQuery = useUserStatusQuery(
    effectivePubkey ? [effectivePubkey] : [],
  );
  const { handleMessage, isOpeningDm } = useProfileDmAction({
    effectivePubkey,
    onClose,
    onOpenDm,
  });
  const isSelf =
    currentPubkey !== undefined &&
    normalizedPubkey.length > 0 &&
    normalizedPubkey === normalizePubkey(currentPubkey);

  useEscapeKey(onClose, isOverlay || isSinglePanelView);
  React.useEffect(() => {
    if (!effectivePubkey) return;
    void profileQuery.refetch();
  }, [effectivePubkey, profileQuery.refetch]);

  const profile = profileQuery.data;
  const displayName =
    profile?.displayName?.trim() ||
    profile?.nip05Handle?.trim() ||
    (effectivePubkey ? truncatePubkey(effectivePubkey) : "Profile");
  const presenceStatus = presenceQuery.data?.[normalizedPubkey];
  const userStatus = userStatusQuery.data?.[normalizedPubkey];
  const profileBody = (
    <AuxiliaryPanelBody className="overflow-y-auto px-4 pb-6">
      <div className="flex flex-col gap-6 pt-4">
        <div className="flex flex-col items-center gap-3 text-center">
          <div className="relative h-20 w-20">
            <ProfileAvatar
              avatarUrl={profile?.avatarUrl ?? null}
              className="h-full w-full text-xl"
              iconClassName="h-8 w-8"
              label={displayName}
              plain
              testId="user-profile-avatar"
            />
            {presenceStatus ? (
              <span
                aria-label={getPresenceLabel(presenceStatus)}
                className="absolute -bottom-0.5 -right-0.5 flex h-6 w-6 items-center justify-center rounded-full bg-background"
                data-testid="user-profile-presence-badge"
                role="img"
              >
                <PresenceDot className="h-3.5 w-3.5" status={presenceStatus} />
              </span>
            ) : null}
          </div>
          <div className="flex flex-col items-center gap-1">
            <h3 className="text-xl font-semibold tracking-tight">
              {displayName}
            </h3>
            {profile?.about?.trim() ? (
              <p
                className="max-w-full px-2 text-center text-sm leading-relaxed text-muted-foreground"
                data-testid="user-profile-description"
              >
                {profile.about.trim()}
              </p>
            ) : null}
            {profile?.nip05Handle ? (
              <p className="text-sm text-muted-foreground">
                {profile.nip05Handle}
              </p>
            ) : null}
            {userStatus ? (
              <p className="text-sm text-muted-foreground">
                {userStatus.emoji ? (
                  <StatusEmoji
                    className="mr-1 inline h-3.5 w-3.5"
                    value={userStatus.emoji}
                  />
                ) : null}
                {userStatus.text}
              </p>
            ) : null}
          </div>
        </div>
        {!isSelf && effectivePubkey && onOpenDm ? (
          <div className="flex items-center justify-center">
            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  aria-label="Message"
                  className={cn(
                    "flex h-12 w-12 items-center justify-center rounded-full bg-muted/60 text-foreground transition-colors hover:bg-muted/80 disabled:cursor-not-allowed disabled:opacity-50",
                  )}
                  data-testid="user-profile-message"
                  disabled={isOpeningDm}
                  onClick={() => void handleMessage()}
                  type="button"
                >
                  {isOpeningDm ? (
                    <Spinner aria-hidden="true" className="h-4 w-4 border-2" />
                  ) : (
                    <MessageSquare className="h-4 w-4" />
                  )}
                </button>
              </TooltipTrigger>
              <TooltipContent align="center" side="top">
                Message
              </TooltipContent>
            </Tooltip>
          </div>
        ) : null}
        {effectivePubkey ? (
          <div className="flex items-center gap-3 rounded-2xl bg-muted/20 px-4 py-3">
            <Fingerprint className="h-4 w-4 shrink-0 text-muted-foreground" />
            <div className="min-w-0 flex-1">
              <div className="text-xs text-muted-foreground">Public key</div>
              <PubKey
                className="text-sm"
                pubkey={effectivePubkey}
                testId="user-profile-copy-pubkey"
              />
            </div>
          </div>
        ) : null}
      </div>
    </AuxiliaryPanelBody>
  );

  return (
    <UserProfilePanelFrame
      addAgentToChannelDialog={null}
      canResetWidth={canResetWidth}
      editAgentDialog={null}
      headerActions={null}
      headerLeftContent={
        <AuxiliaryPanelHeaderGroup align="center">
          <AuxiliaryPanelHeaderTitleBlock title="Profile" />
        </AuxiliaryPanelHeaderGroup>
      }
      isOverlay={isOverlay}
      isSinglePanelView={isSinglePanelView}
      isSplitLayout={isSplitLayout}
      onClose={onClose}
      onResetWidth={onResetWidth}
      onResizeStart={onResizeStart}
      personaDialogs={null}
      profileBody={profileBody}
      splitPaneClamp={splitPaneClamp}
      transparentChrome={transparentChrome}
      widthPx={widthPx}
    />
  );
}
