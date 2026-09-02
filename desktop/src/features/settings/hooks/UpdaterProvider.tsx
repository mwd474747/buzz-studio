import { createContext, useContext, type ReactNode } from "react";
import { useLocalOwnerPolicy } from "@/features/onboarding/useLocalOwnerPolicy";
import { useUpdater } from "./use-updater";

type UpdaterContextValue = ReturnType<typeof useUpdater>;

const UpdaterContext = createContext<UpdaterContextValue | null>(null);

export function UpdaterProvider({ children }: { children: ReactNode }) {
  const localOwnerPolicy = useLocalOwnerPolicy();
  const updater = useUpdater(localOwnerPolicy === "inactive");
  return <UpdaterContext value={updater}>{children}</UpdaterContext>;
}

export function useUpdaterContext(): UpdaterContextValue {
  const ctx = useContext(UpdaterContext);
  if (!ctx) {
    throw new Error("useUpdaterContext must be used within an UpdaterProvider");
  }
  return ctx;
}
