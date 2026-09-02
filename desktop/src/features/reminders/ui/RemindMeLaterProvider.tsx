import * as React from "react";

import { useRemindersQuery } from "@/features/reminders/hooks";
import type { ReminderTarget } from "@/features/reminders/lib/reminderTypes";
import { RemindMeLaterDialog } from "./RemindMeLaterDialog";

type RemindMeLaterContextValue = {
  enabled: boolean;
  openReminder: (target: ReminderTarget) => void;
  /** Event IDs of messages with a pending reminder, for channel tinting. */
  activeReminderEventIds: ReadonlySet<string>;
};

const RemindMeLaterContext = React.createContext<RemindMeLaterContextValue>({
  enabled: false,
  openReminder: () => {},
  activeReminderEventIds: new Set(),
});

export function useRemindLater() {
  return React.useContext(RemindMeLaterContext);
}

export function RemindMeLaterProvider({
  enabled = true,
  pubkey,
  children,
}: {
  enabled?: boolean;
  pubkey?: string;
  children: React.ReactNode;
}) {
  if (!enabled) {
    return children;
  }
  return (
    <ActiveRemindMeLaterProvider pubkey={pubkey}>
      {children}
    </ActiveRemindMeLaterProvider>
  );
}

function ActiveRemindMeLaterProvider({
  pubkey,
  children,
}: {
  pubkey?: string;
  children: React.ReactNode;
}) {
  const [open, setOpen] = React.useState(false);
  const [target, setTarget] = React.useState<ReminderTarget | null>(null);

  const openReminder = React.useCallback((t: ReminderTarget) => {
    setTarget(t);
    setOpen(true);
  }, []);

  const remindersQuery = useRemindersQuery(pubkey);
  const reminders = remindersQuery.data;
  const activeReminderEventIds = React.useMemo(() => {
    const ids = new Set<string>();
    for (const reminder of reminders ?? []) {
      if (
        reminder.content.status === "pending" &&
        reminder.content.target?.eventId
      ) {
        ids.add(reminder.content.target.eventId);
      }
    }
    return ids;
  }, [reminders]);

  const contextValue = React.useMemo(
    () => ({ enabled: true, openReminder, activeReminderEventIds }),
    [openReminder, activeReminderEventIds],
  );

  return (
    <RemindMeLaterContext.Provider value={contextValue}>
      {children}
      <RemindMeLaterDialog open={open} onOpenChange={setOpen} target={target} />
    </RemindMeLaterContext.Provider>
  );
}
