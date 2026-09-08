import type * as React from "react";

import { AppHuddleBar } from "@/app/AppHuddleBar";
import { AppShellTrayMenu } from "@/app/useAppShellTrayMenu";
import type { useAppNavigation } from "@/app/navigation/useAppNavigation";
import { PreventSleepProvider } from "@/features/agents/usePreventSleep";
import { HuddleProvider } from "@/features/huddle";
import type { NotificationSettings } from "@/features/notifications/hooks";
import { RemindMeLaterProvider } from "@/features/reminders/ui/RemindMeLaterProvider";
import { useReminderNotifications } from "@/features/reminders/useReminderNotifications";
import type { Channel } from "@/shared/api/types";

type GoChannel = ReturnType<typeof useAppNavigation>["goChannel"];

type OrdinaryAppShellBoundaryProps = {
  channels: Channel[];
  children: React.ReactNode;
  enabled: boolean;
  goChannel: GoChannel;
  notificationSettings: NotificationSettings;
  onHuddleVisibilityChange: (open: boolean) => void;
  openCreateChannel: () => void;
  pubkey?: string;
};

/** Ordinary-product background and agent providers excluded from local-owner. */
export function OrdinaryAppShellBoundary({
  channels,
  children,
  enabled,
  goChannel,
  notificationSettings,
  onHuddleVisibilityChange,
  openCreateChannel,
  pubkey,
}: OrdinaryAppShellBoundaryProps) {
  useReminderNotifications(enabled ? pubkey : undefined, notificationSettings, channels);

  return (
    <PreventSleepProvider enabled={enabled}>
      <AppShellTrayMenu
        channels={channels}
        enabled={enabled}
        goChannel={goChannel}
        openCreateChannel={openCreateChannel}
      />
      <HuddleProvider enabled={enabled}>
        <RemindMeLaterProvider enabled={enabled} pubkey={pubkey}>
          {children}
          {enabled ? (
            <div className="absolute inset-x-0 bottom-0 z-0 h-(--buzz-huddle-drawer-height)">
              <AppHuddleBar
                onOpenThread={(channelId, messageId) => {
                  void goChannel(channelId, {
                    messageId,
                    threadRootId: messageId,
                  });
                }}
                onVisibilityChange={onHuddleVisibilityChange}
              />
            </div>
          ) : null}
        </RemindMeLaterProvider>
      </HuddleProvider>
    </PreventSleepProvider>
  );
}
