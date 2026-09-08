import { Fingerprint, MessageSquare } from "lucide-react";
import * as React from "react";

import { usePresenceQuery } from "@/features/presence/hooks";
import { useUserProfileQuery } from "@/features/profile/hooks";
import { ProfileQuickAction } from "@/features/profile/ui/ProfileQuickAction";
import { UserProfileHero } from "@/features/profile/ui/UserProfileHero";
import { UserProfilePanelFrame } from "@/features/profile/ui/UserProfilePanelFrame";
import type { UserProfilePanelProps } from "@/features/profile/ui/UserProfilePanelUtils";
import { useProfileDmAction } from "@/features/profile/ui/useProfileDmAction";
import { useUserStatusQuery } from "@/features/user-status/hooks";
import { useEscapeKey } from "@/shared/hooks/useEscapeKey";
import { useIsThreadPanelOverlay } from "@/shared/hooks/use-mobile";
import {
  AuxiliaryPanelBody,
  AuxiliaryPanelHeaderGroup,
  AuxiliaryPanelHeaderTitleBlock,
} from "@/shared/layout/AuxiliaryPanel";
import { normalizePubkey, truncatePubkey } from "@/shared/lib/pubkey";
import { PubKey } from "@/shared/ui/PubKey";

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
  const profileBody = (
    <AuxiliaryPanelBody className="overflow-y-auto px-4 pb-6">
      <div className="flex flex-col gap-6 pt-4">
        <UserProfileHero
          displayName={displayName}
          presenceStatus={presenceQuery.data?.[normalizedPubkey]}
          profile={profile}
          userStatus={userStatusQuery.data?.[normalizedPubkey]}
        />
        {!isSelf && effectivePubkey && onOpenDm ? (
          <div className="flex items-center justify-center">
            <ProfileQuickAction
              disabled={isOpeningDm}
              icon={MessageSquare}
              isLoading={isOpeningDm}
              label="Message"
              onClick={() => void handleMessage()}
              testId="user-profile-message"
            />
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
      canResetWidth={canResetWidth}
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
      profileBody={profileBody}
      siblings={null}
      splitPaneClamp={splitPaneClamp}
      transparentChrome={transparentChrome}
      widthPx={widthPx}
    />
  );
}
