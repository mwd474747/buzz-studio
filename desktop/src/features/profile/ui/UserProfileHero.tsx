import * as React from "react";
import { ChevronDown, ChevronUp } from "lucide-react";

import { getPresenceLabel } from "@/features/presence/lib/presence";
import { PresenceDot } from "@/features/presence/ui/PresenceBadge";
import type { useUserProfileQuery } from "@/features/profile/hooks";
import {
  MaskedAvatarBadgeFrame,
  STATUS_DOT_MASK_CURVE,
} from "@/features/profile/ui/MaskedAvatarBadgeFrame";
import { ProfileAvatar } from "@/features/profile/ui/ProfileAvatar";
import { StatusEmoji } from "@/features/user-status/ui/StatusEmoji";
import { cn } from "@/shared/lib/cn";

const PROFILE_HERO_SPACING = {
  "0": 0,
  "6": 24,
} as const;

const PROFILE_HERO_PRESENCE_BADGE = {
  cutout: { cx: 68, cy: 68, r: 15 },
  shell: {
    bottom: PROFILE_HERO_SPACING["0"],
    height: PROFILE_HERO_SPACING["6"],
    right: PROFILE_HERO_SPACING["0"],
    width: PROFILE_HERO_SPACING["6"],
  },
} as const;

/** Human-only profile hero for the local-owner surface. */
export function UserProfileHero({
  displayName,
  presenceStatus,
  profile,
  userStatus,
}: {
  displayName: string;
  presenceStatus: "online" | "away" | "offline" | undefined;
  profile: ReturnType<typeof useUserProfileQuery>["data"];
  userStatus: { text: string; emoji: string } | null | undefined;
}) {
  return (
    <div className="flex flex-col items-center gap-3 text-center">
      <MaskedAvatarBadgeFrame
        badge={
          presenceStatus ? (
            <span
              aria-label={getPresenceLabel(presenceStatus)}
              className="flex h-6 w-6 items-center justify-center rounded-full"
              data-testid="user-profile-presence-badge"
              role="img"
            >
              <PresenceDot className="h-3.5 w-3.5" status={presenceStatus} />
            </span>
          ) : null
        }
        badgeBox={PROFILE_HERO_PRESENCE_BADGE.shell}
        className="h-20 w-20"
        curve={STATUS_DOT_MASK_CURVE}
        cutout={PROFILE_HERO_PRESENCE_BADGE.cutout}
        size={80}
      >
        <ProfileAvatar
          avatarUrl={profile?.avatarUrl ?? null}
          className="h-full w-full text-xl"
          iconClassName="h-8 w-8"
          label={displayName}
          plain
          testId="user-profile-avatar"
        />
      </MaskedAvatarBadgeFrame>

      <div className="flex flex-col items-center gap-1">
        <h3 className="text-xl font-semibold tracking-tight">{displayName}</h3>

        {profile?.about?.trim() ? (
          <UserProfileHeroDescription
            about={profile.about.trim()}
            key={profile.about.trim()}
          />
        ) : null}

        {profile?.nip05Handle ? (
          <p className="text-sm text-muted-foreground">{profile.nip05Handle}</p>
        ) : null}

        {userStatus ? (
          <p className="text-sm text-muted-foreground">
            {userStatus.emoji ? (
              <StatusEmoji
                className="mr-1 inline h-3.5 w-3.5"
                value={userStatus.emoji}
              />
            ) : null}
            {userStatus.text}
          </p>
        ) : null}
      </div>
    </div>
  );
}

function UserProfileHeroDescription({ about }: { about: string }) {
  const [expanded, setExpanded] = React.useState(false);
  const [isTruncated, setIsTruncated] = React.useState(false);
  const textRef = React.useRef<HTMLParagraphElement>(null);

  const measureTruncation = React.useCallback(() => {
    const element = textRef.current;
    if (!element || expanded) {
      return;
    }
    setIsTruncated(element.scrollHeight > element.clientHeight + 1);
  }, [expanded]);

  React.useLayoutEffect(() => {
    measureTruncation();
  }, [measureTruncation]);

  React.useEffect(() => {
    const element = textRef.current;
    if (!element) {
      return;
    }

    const observer = new ResizeObserver(() => {
      measureTruncation();
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [measureTruncation]);

  const toggleClassName =
    "inline-flex items-center gap-0.5 text-xs font-medium text-muted-foreground opacity-60 transition-opacity hover:text-foreground hover:opacity-100";

  return (
    <div className="flex w-full flex-col items-center gap-0.5">
      <div className="w-fit max-w-full px-2">
        <p
          className={cn(
            "text-center whitespace-pre-wrap text-sm leading-relaxed text-muted-foreground",
            !expanded && "line-clamp-3",
          )}
          data-testid="user-profile-description"
          ref={textRef}
        >
          {about}
        </p>
      </div>
      {!expanded && isTruncated ? (
        <button
          className={toggleClassName}
          data-testid="user-profile-description-toggle"
          onClick={() => setExpanded(true)}
          type="button"
        >
          more
          <ChevronDown className="h-4 w-4" />
        </button>
      ) : null}
      {expanded ? (
        <button
          className={toggleClassName}
          data-testid="user-profile-description-toggle"
          onClick={() => setExpanded(false)}
          type="button"
        >
          less
          <ChevronUp className="h-4 w-4" />
        </button>
      ) : null}
    </div>
  );
}
