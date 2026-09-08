import * as React from "react";

import type { UserProfilePanelProps } from "@/features/profile/ui/UserProfilePanelUtils";

const ProductUserProfilePanel =
  import.meta.env.MODE === "local-owner"
    ? React.lazy(async () => {
        const module = await import(
          "@/features/profile/ui/LocalOwnerUserProfilePanel"
        );
        return { default: module.LocalOwnerUserProfilePanel };
      })
    : React.lazy(async () => {
        const module = await import(
          "@/features/profile/ui/OrdinaryUserProfilePanel"
        );
        return { default: module.OrdinaryUserProfilePanel };
      });

export type {
  ProfilePanelTab,
  ProfilePanelView,
} from "@/features/profile/ui/UserProfilePanelUtils";

/** Build-selected profile surface: human-only locally, full product otherwise. */
export function UserProfilePanel(props: UserProfilePanelProps) {
  return (
    <React.Suspense fallback={null}>
      <ProductUserProfilePanel {...props} />
    </React.Suspense>
  );
}
