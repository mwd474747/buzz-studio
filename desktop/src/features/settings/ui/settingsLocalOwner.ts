import type { LocalOwnerPolicyStatus } from "@/features/onboarding/useLocalOwnerPolicy";

import type { SettingsSection } from "./SettingsPanels";

/** Settings that remain on the compiled local-owner cockpit. */
export const LOCAL_OWNER_SETTINGS = new Set<SettingsSection>([
  "profile",
  "appearance",
  "notifications",
  "shortcuts",
  "custom-emoji",
]);

/**
 * Hide retired planes only after the native profile reports active.
 *
 * `loading` and `unavailable` must not subtract Settings → Agents / Members.
 * A remount during an in-flight lookup used to treat those states as
 * local-owner and rewrite the section away from the requested page.
 */
export function isSettingsSectionAllowedForLocalOwnerPolicy(
  policy: LocalOwnerPolicyStatus,
  section: SettingsSection,
): boolean {
  return policy !== "active" || LOCAL_OWNER_SETTINGS.has(section);
}
