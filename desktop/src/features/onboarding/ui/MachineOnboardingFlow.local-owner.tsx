import type { QueryClient } from "@tanstack/react-query";

import { importIdentity } from "@/shared/api/tauriIdentity";
import { StartupWindowDragRegion } from "@/shared/ui/StartupWindowDragRegion";
import { NostrKeyImportForm } from "@/features/onboarding/ui/NostrKeyImportForm";
import { OnboardingChrome } from "./OnboardingChrome";
import { OnboardingFooterProvider } from "./OnboardingFooter";

export type MachineOnboardingPage =
  | "identity"
  | "key-import"
  | "backup"
  | "setup"
  | "config";

export type PostOnboardingNavigation = {
  to: string;
  search?: Record<string, string>;
};

type LocalOwnerMachineOnboardingProps = {
  complete: (pubkey?: string) => void;
  continueWithIdentity: (pubkey: string) => void;
  identityLost: boolean;
  initialPage?: MachineOnboardingPage;
  navigateAfterComplete?: (nav: PostOnboardingNavigation) => void;
  queryClient: QueryClient;
};

/** The pinned product can only recover its existing owner identity. */
export function MachineOnboardingFlow(props: LocalOwnerMachineOnboardingProps) {
  const importOwnerIdentity = async (nsec: string, password?: string) => {
    const identity = await importIdentity(nsec, password);
    props.continueWithIdentity(identity.pubkey);
    props.queryClient.setQueryData(["identity"], identity);
  };

  return (
    <div
      className="buzz-onboarding-neutral-theme buzz-startup-shell flex max-h-dvh items-start justify-center overflow-x-hidden overflow-y-auto px-4 pb-28 pt-[106px] text-foreground"
      data-testid="machine-onboarding-gate"
    >
      <StartupWindowDragRegion />
      <OnboardingChrome current={2} total={2} />
      <OnboardingFooterProvider>
        <div className="relative flex min-h-[calc(100dvh-13.25rem)] w-full max-w-[837px] flex-col items-center text-center">
          <h1 className="text-title font-normal text-foreground">
            Re-import your owner key
          </h1>
          <p className="mt-5 max-w-[460px] text-sm leading-6 text-foreground/80">
            This installation is pinned to its existing owner identity. Import
            the matching nsec to restore access; replacement identities are
            disabled.
          </p>
          <div className="w-full max-w-[440px]">
            <NostrKeyImportForm
              onBack={() => undefined}
              onImport={importOwnerIdentity}
              showBack={false}
              variant="spotlight"
            />
          </div>
        </div>
      </OnboardingFooterProvider>
    </div>
  );
}
