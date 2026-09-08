import type { ReactNode } from "react";

import { router } from "@/app/router";
import { KnownAgentPubkeysProvider } from "@/features/agents/useKnownAgentPubkeys";
import { EncryptedBackupProvider } from "@/features/settings/EncryptedBackupProvider";

/** Providers used by the ordinary product but absent from local-owner builds. */
export function OrdinaryAppBoundary({
  children,
  enabled,
}: {
  children: ReactNode;
  enabled: boolean;
}) {
  return (
    <EncryptedBackupProvider
      onOpenSettings={() =>
        void router.navigate({
          to: "/settings",
          search: { section: "profile" },
        })
      }
    >
      <KnownAgentPubkeysProvider enabled={enabled}>
        {children}
      </KnownAgentPubkeysProvider>
    </EncryptedBackupProvider>
  );
}
