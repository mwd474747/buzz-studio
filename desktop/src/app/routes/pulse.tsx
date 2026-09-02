import * as React from "react";
import { createFileRoute, Navigate } from "@tanstack/react-router";

import {
  parseProfilePanelTab,
  parseProfilePanelView,
  type ProfilePanelTab,
  type ProfilePanelView,
} from "@/features/profile/ui/UserProfilePanelUtils";
import { usePreviewFeatureWarning } from "@/shared/features";
import { ViewLoadingFallback } from "@/shared/ui/ViewLoadingFallback";
import { useLocalOwnerPolicy } from "@/features/onboarding/useLocalOwnerPolicy";

const PulseScreen = React.lazy(async () => {
  const module = await import("@/features/pulse/ui/PulseScreen");
  return { default: module.PulseScreen };
});

type PulseRouteSearch = {
  profile?: string;
  profileTab?: ProfilePanelTab;
  profileView?: ProfilePanelView;
};

function validatePulseSearch(
  search: Record<string, unknown>,
): PulseRouteSearch {
  return {
    profile:
      typeof search.profile === "string" && search.profile.length > 0
        ? search.profile
        : undefined,
    profileTab: parseProfilePanelTab(search.profileTab) ?? undefined,
    profileView: parseProfilePanelView(search.profileView) ?? undefined,
  };
}

export const Route = createFileRoute("/pulse")({
  validateSearch: validatePulseSearch,
  component: PulseRouteComponent,
});

function PulseRouteComponent() {
  const localOwnerPolicy = useLocalOwnerPolicy();
  usePreviewFeatureWarning("pulse");
  if (localOwnerPolicy === "loading") {
    return <ViewLoadingFallback includeHeader kind="pulse" />;
  }
  if (localOwnerPolicy !== "inactive") {
    return <Navigate replace to="/" />;
  }
  return (
    <React.Suspense
      fallback={<ViewLoadingFallback includeHeader kind="pulse" />}
    >
      <PulseScreen />
    </React.Suspense>
  );
}
