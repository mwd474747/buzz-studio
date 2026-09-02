import { createPortal } from "react-dom";

import type {
  AvatarEditorPresentation,
  AvatarMode,
} from "@/features/profile/ui/ProfileAvatarEditor.types";
import { cn } from "@/shared/lib/cn";
import { Tabs, TabsList, TabsTrigger } from "@/shared/ui/tabs";

const MODE_TAB_ORDER: AvatarMode[] = ["image", "emoji", "animated"];
const MODE_TAB_LABELS: Record<AvatarMode, string> = {
  animated: "Animated",
  emoji: "Emoji",
  image: "Image",
};

type ProfileAvatarModeTabsProps = {
  disabled: boolean;
  mode: AvatarMode;
  onModeChange: (mode: AvatarMode) => void;
  presentation: AvatarEditorPresentation;
  portalContainer?: HTMLElement | null;
  animatedModeEnabled?: boolean;
};

export function ProfileAvatarModeTabs({
  disabled,
  mode,
  onModeChange,
  presentation,
  portalContainer,
  animatedModeEnabled = true,
}: ProfileAvatarModeTabsProps) {
  const isOnboardingModal = presentation === "onboarding-modal";
  const modes = animatedModeEnabled
    ? MODE_TAB_ORDER
    : MODE_TAB_ORDER.filter((mode) => mode !== "animated");
  const tabs = (
    <Tabs
      className={isOnboardingModal ? "flex w-full justify-center" : "w-full"}
      onValueChange={(nextMode) => {
        if (!disabled) onModeChange(nextMode as AvatarMode);
      }}
      value={mode}
    >
      <TabsList
        aria-label="Avatar type"
        className={cn(
          isOnboardingModal
            ? "flex h-10 w-auto gap-2 rounded-none bg-transparent p-0 text-muted-foreground"
            : cn(
                "relative isolate grid h-14 w-full overflow-hidden rounded-full bg-muted p-1 text-muted-foreground",
                animatedModeEnabled ? "grid-cols-3" : "grid-cols-2",
              ),
        )}
      >
        {isOnboardingModal ? null : (
          <div
            aria-hidden="true"
            className="absolute bottom-1 left-1 top-1 z-0 rounded-full bg-background shadow transition-transform duration-[250ms] ease-out"
            style={{
              transform: `translateX(${modes.indexOf(mode) * 100}%)`,
              width: `calc((100% - 8px) / ${modes.length})`,
            }}
          />
        )}
        {modes.map((tabMode) => (
          <TabsTrigger
            className={cn(
              isOnboardingModal
                ? "relative z-10 h-10 rounded-[6px] px-4 text-sm font-normal shadow-none transition-colors data-[state=active]:bg-[rgb(var(--buzz-onboarding-avatar-action-bg))] data-[state=active]:text-[rgb(var(--buzz-onboarding-avatar-action-fg))] data-[state=active]:shadow-none"
                : "relative z-10 h-full rounded-full bg-transparent text-sm font-medium shadow-none transition-colors data-[state=active]:bg-transparent data-[state=active]:text-foreground data-[state=active]:shadow-none",
            )}
            disabled={disabled}
            key={tabMode}
            value={tabMode}
          >
            {MODE_TAB_LABELS[tabMode]}
          </TabsTrigger>
        ))}
      </TabsList>
    </Tabs>
  );

  return portalContainer === undefined
    ? tabs
    : portalContainer
      ? createPortal(tabs, portalContainer)
      : null;
}
