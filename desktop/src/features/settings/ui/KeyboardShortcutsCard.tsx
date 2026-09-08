import {
  getShortcutsByCategory,
  getPlatformKeys,
  type KeyboardShortcut,
} from "@/shared/lib/keyboard-shortcuts";
import { useLocalOwnerPolicy } from "@/features/onboarding/useLocalOwnerPolicy";
import { SettingsOptionGroup, SettingsOptionRow } from "./SettingsOptionGroup";
import { SettingsSectionHeader } from "./SettingsSectionHeader";

function KeyCombo({ shortcut }: { shortcut: KeyboardShortcut }) {
  const keys = getPlatformKeys(shortcut);
  // Split on "+" but keep "+" as a standalone key (e.g. for zoom-in "⌘+")
  const parts = keys
    .split(/(?<!\+)\+(?!\s*$)/)
    .map((p) => p.trim())
    .filter(Boolean);

  return (
    <span className="flex items-center gap-1">
      {parts.map((part) => (
        <kbd
          className="inline-flex h-6 min-w-6 items-center justify-center rounded border border-border/70 bg-muted/60 px-1.5 font-mono text-xs text-muted-foreground"
          key={part}
        >
          {part}
        </kbd>
      ))}
    </span>
  );
}

export function KeyboardShortcutsCard() {
  const categories = getShortcutsByCategory();
  if (useLocalOwnerPolicy() !== "inactive") {
    const omitted = new Set([
      "browse-channels",
      "new-channel",
      "publish-note",
      "push-to-talk",
    ]);
    for (const [category, shortcuts] of categories) {
      categories.set(
        category,
        shortcuts.filter((shortcut) => !omitted.has(shortcut.id)),
      );
    }
  }

  return (
    <section className="min-w-0" data-testid="settings-shortcuts">
      <SettingsSectionHeader
        title="Keyboard shortcuts"
        description="All available keyboard shortcuts. Shortcuts are read-only."
      />

      <div className="space-y-4">
        {[...categories.entries()].map(([category, shortcuts]) => (
          <div key={category}>
            <h2 className="mb-2 text-lg font-semibold tracking-tight">
              {category}
            </h2>
            <SettingsOptionGroup>
              {shortcuts.map((shortcut) => (
                <SettingsOptionRow
                  className="min-h-12 px-3 py-2"
                  key={shortcut.id}
                >
                  <div className="min-w-0 flex-1">
                    <span className="text-sm font-medium text-foreground">
                      {shortcut.label}
                    </span>
                    <span className="ml-2 text-muted-foreground">
                      {shortcut.description}
                    </span>
                  </div>
                  <KeyCombo shortcut={shortcut} />
                </SettingsOptionRow>
              ))}
            </SettingsOptionGroup>
          </div>
        ))}
      </div>
    </section>
  );
}
