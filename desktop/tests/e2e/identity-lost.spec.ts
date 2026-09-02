import { hexToBytes } from "@noble/hashes/utils.js";
import { expect, test } from "@playwright/test";
import { nsecEncode } from "nostr-tools/nip19";

import { installMockBridge, TEST_IDENTITIES } from "../helpers/bridge";
import { openSettings } from "../helpers/settings";

function localOwnerProfile(ownerPubkey: string) {
  return {
    profile: "local-owner",
    profile_sha256: "1".repeat(64),
    source_commit: "dce3cb57ef760b2b4837fb75bf6ab5bde879bd66",
    source_tree: "2".repeat(40),
    bundle_identifier: "xyz.block.buzz.app",
    keyring_service: "buzz-desktop",
    relay_ws_url: "ws://localhost:3300",
    owner_pubkey: ownerPubkey,
    owner_pubkey_sha256: "3".repeat(64),
    macos_signing_required: true,
    macos_signing_configured: false,
  };
}

test("normal first launch uses the already-persisted identity", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await installMockBridge(page, undefined, {
    skipCommunitySeed: true,
    skipOnboardingSeed: true,
  });
  await page.goto("/");

  const gate = page.getByTestId("machine-onboarding-gate");
  await expect(gate).toBeVisible();
  await expect(gate).toHaveCSS("background-color", "rgb(215, 215, 46)");
  // Landing carries a subtle dot-grid pattern over the chartreuse fill.
  await expect(gate).toHaveCSS("background-image", /radial-gradient/);
  await expect(gate).toHaveCSS("color", "rgb(23, 23, 23)");
  await expect(
    page.getByRole("button", { name: "Create a new identity key" }),
  ).toHaveCSS("background-color", "rgb(23, 23, 23)");
  await page.getByRole("button", { name: "Create a new identity key" }).click();

  await expect(
    page.getByRole("heading", {
      name: "Your unique identity key has been created",
    }),
  ).toBeVisible();
  // Non-landing pages layer the dot grid over the chartreuse→light-blue gradient.
  await expect(gate).toHaveCSS(
    "background-image",
    /radial-gradient\(.*\), linear-gradient\(.*rgb\(215, 215, 46\).*rgb\(215, 231, 246\)\)/s,
  );
  await expect(gate).toHaveCSS("color", "rgb(23, 23, 23)");
  const commands = await page.evaluate(
    () =>
      (
        window as Window & {
          __BUZZ_E2E_COMMAND_PAYLOADS__?: Array<{ command: string }>;
        }
      ).__BUZZ_E2E_COMMAND_PAYLOADS__ ?? [],
  );
  expect(commands.some((entry) => entry.command === "get_identity")).toBe(true);
  expect(
    commands.some((entry) => entry.command === "persist_current_identity"),
  ).toBe(false);
});

test("local-owner profile bootstraps its pinned localhost relay for the exact owner", async ({
  page,
}) => {
  const ownerPubkey = "deadbeef".repeat(8);
  await installMockBridge(
    page,
    { localOwnerProfile: localOwnerProfile(ownerPubkey) },
    {
      autoConnectDefaultRelay: true,
      relayWsUrl: "ws://localhost:3300",
      skipCommunitySeed: true,
    },
  );
  await page.goto("/");

  await expect
    .poll(() =>
      page.evaluate(() => {
        const raw = window.localStorage.getItem("buzz-communities");
        return raw ? JSON.parse(raw) : null;
      }),
    )
    .toEqual([
      expect.objectContaining({
        name: "Local Dev",
        pubkey: ownerPubkey,
        relayUrl: "ws://localhost:3300",
      }),
    ]);
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (
            window as Window & {
              __BUZZ_E2E_COMMAND_PAYLOADS__?: Array<{ command: string }>;
            }
          ).__BUZZ_E2E_COMMAND_PAYLOADS__?.some(
            (entry) => entry.command === "apply_workspace",
          ) ?? false,
      ),
    )
    .toBe(true);
  await expect(page.getByTestId("machine-onboarding-gate")).toHaveCount(0);

  await openSettings(page, "profile");
  await page.getByTestId("profile-avatar-edit").click();
  await expect(page.getByRole("tab", { name: "Image" })).toBeVisible();
  await expect(page.getByRole("tab", { name: "Emoji" })).toBeVisible();
  await expect(page.getByRole("tab", { name: "Animated" })).toHaveCount(0);
});

for (const stage of ["connecting", "claiming"] as const) {
  test(`local-owner discards a persisted ${stage} alternate-community transaction`, async ({
    page,
  }) => {
    const ownerPubkey = "deadbeef".repeat(8);
    await installMockBridge(
      page,
      { localOwnerProfile: localOwnerProfile(ownerPubkey) },
      {
        autoConnectDefaultRelay: true,
        relayWsUrl: "ws://localhost:3300",
        skipCommunitySeed: true,
      },
    );
    await page.addInitScript((persistedStage) => {
      const timestamp = new Date().toISOString();
      window.localStorage.setItem(
        "buzz-community-onboarding-transaction.v1",
        JSON.stringify({
          id: `stale-${persistedStage}`,
          source: "deep-link-join",
          stage: persistedStage,
          relayUrl: "wss://retired.example",
          inviteCode: persistedStage === "claiming" ? "retired" : undefined,
          communityName: "Retired",
          token: "legacy-token",
          reposDir: "/tmp/legacy-repos",
          createdAt: timestamp,
          updatedAt: timestamp,
        }),
      );
    }, stage);
    await page.goto("/");

    await expect
      .poll(() =>
        page.evaluate(() =>
          window.localStorage.getItem(
            "buzz-community-onboarding-transaction.v1",
          ),
        ),
      )
      .toBeNull();
    await expect
      .poll(() =>
        page.evaluate(() => {
          const raw = window.localStorage.getItem("buzz-communities");
          return raw ? JSON.parse(raw) : null;
        }),
      )
      .toEqual([
        expect.objectContaining({
          name: "Local Dev",
          pubkey: ownerPubkey,
          relayUrl: "ws://localhost:3300",
        }),
      ]);
    await expect(page.getByText("Retired", { exact: true })).toHaveCount(0);
  });
}

test("local-owner profile replaces stale community state before backend apply", async ({
  page,
}) => {
  const ownerPubkey = "deadbeef".repeat(8);
  await installMockBridge(
    page,
    { localOwnerProfile: localOwnerProfile(ownerPubkey) },
    { skipCommunitySeed: true },
  );
  await page.addInitScript(() => {
    if (window.sessionStorage.getItem("local-owner-stale-seeded")) return;
    window.sessionStorage.setItem("local-owner-stale-seeded", "1");
    window.localStorage.setItem(
      "buzz-communities",
      JSON.stringify([
        {
          id: "legacy-community",
          name: "Legacy",
          relayUrl: "ws://localhost:3000",
          pubkey: "wrong-owner",
          token: "retired-token",
          reposDir: "/tmp/retired-agent-repos",
          addedAt: "2026-08-01T00:00:00.000Z",
        },
      ]),
    );
    window.localStorage.setItem("buzz-active-community-id", "legacy-community");
  });
  await page.goto("/");

  await expect
    .poll(() =>
      page.evaluate(() => {
        const raw = window.localStorage.getItem("buzz-communities");
        return raw ? JSON.parse(raw) : null;
      }),
    )
    .toEqual([
      expect.objectContaining({
        name: "Local Dev",
        pubkey: ownerPubkey,
        relayUrl: "ws://localhost:3300",
      }),
    ]);
  const workspaceCalls = await page.evaluate(
    () =>
      (
        window as Window & {
          __BUZZ_E2E_COMMAND_PAYLOADS__?: Array<{
            command: string;
            payload?: Record<string, unknown>;
          }>;
        }
      ).__BUZZ_E2E_COMMAND_PAYLOADS__?.filter(
        (entry) => entry.command === "apply_workspace",
      ) ?? [],
  );
  expect(workspaceCalls).toHaveLength(1);
  expect(workspaceCalls[0]?.payload?.relayUrl).toBe("ws://localhost:3300");
  expect(workspaceCalls[0]?.payload?.agentManagedProfiles).toBe(false);
});

test("lost boot opens onboarding gate directly on the key-import page", async ({
  page,
}) => {
  await installMockBridge(
    page,
    { identityLost: true },
    { skipOnboardingSeed: true },
  );
  await page.goto("/");

  await expect(page.getByTestId("machine-onboarding-gate")).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Re-import your key" }),
  ).toBeVisible();
});

test("local-owner lost recovery refuses replacement and accepts only the pinned owner", async ({
  page,
}) => {
  await installMockBridge(
    page,
    {
      identityLost: true,
      localOwnerProfile: localOwnerProfile(TEST_IDENTITIES.alice.pubkey),
    },
    { skipOnboardingSeed: true },
  );
  await page.goto("/");

  await expect(
    page.getByText(
      "This installation is pinned to its existing owner identity.",
      { exact: false },
    ),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Start new identity" }),
  ).toHaveCount(0);

  const wrongNsec = nsecEncode(hexToBytes(TEST_IDENTITIES.bob.privateKey));
  await page.getByTestId("nostr-import-nsec-input").fill(wrongNsec);
  await page.getByTestId("nostr-import-submit").click();
  await expect(page.getByTestId("nostr-import-feedback")).toContainText(
    "local-owner build requires the ratified #local-dev owner identity",
  );
  await expect(page.getByTestId("relaunch-required")).toHaveCount(0);

  const ownerNsec = nsecEncode(hexToBytes(TEST_IDENTITIES.alice.privateKey));
  await page.getByTestId("nostr-import-nsec-input").fill(ownerNsec);
  await page.getByTestId("nostr-import-submit").click();
  await expect(page.getByTestId("relaunch-required")).toBeVisible();
});

test("importing a key from lost mode shows the relaunch-required screen", async ({
  page,
}) => {
  await installMockBridge(
    page,
    { identityLost: true },
    { skipOnboardingSeed: true },
  );
  await page.goto("/");

  await expect(
    page.getByRole("heading", { name: "Re-import your key" }),
  ).toBeVisible();

  const importedNsec = nsecEncode(hexToBytes(TEST_IDENTITIES.alice.privateKey));
  await page.getByTestId("nostr-import-nsec-input").fill(importedNsec);
  await expect(page.getByTestId("nostr-import-npub-preview")).toBeVisible();
  await page.getByTestId("nostr-import-submit").click();

  await expect(page.getByTestId("relaunch-required")).toBeVisible();
});

test("start-new-identity from lost mode persists the ephemeral key after confirmation", async ({
  page,
}) => {
  await installMockBridge(
    page,
    { identityLost: true },
    { skipOnboardingSeed: true },
  );
  await page.goto("/");

  await expect(
    page.getByRole("heading", { name: "Re-import your key" }),
  ).toBeVisible();

  page.on("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Start new identity" }).click();

  await expect(page.getByTestId("relaunch-required")).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (
            window as Window & {
              __BUZZ_E2E_COMMAND_PAYLOADS__?: Array<{ command: string }>;
            }
          ).__BUZZ_E2E_COMMAND_PAYLOADS__?.some(
            (e) => e.command === "persist_current_identity",
          ) ?? false,
      ),
    )
    .toBe(true);
});

test("cancelling start-new-identity in lost mode stays on the import screen", async ({
  page,
}) => {
  await installMockBridge(
    page,
    { identityLost: true },
    { skipOnboardingSeed: true },
  );
  await page.goto("/");

  await expect(
    page.getByRole("heading", { name: "Re-import your key" }),
  ).toBeVisible();

  page.on("dialog", (dialog) => dialog.dismiss());
  await page.getByRole("button", { name: "Start new identity" }).click();

  // Still on the import screen — no navigation, no persist
  await expect(
    page.getByRole("heading", { name: "Re-import your key" }),
  ).toBeVisible();
  await expect(page.getByTestId("relaunch-required")).toHaveCount(0);
});

test("locked boot shows the keyring-locked screen without the onboarding gate or key-import UI", async ({
  page,
}) => {
  await installMockBridge(
    page,
    { identityLocked: true },
    { skipOnboardingSeed: true },
  );
  await page.goto("/");

  await expect(page.getByTestId("keyring-locked")).toBeVisible();
  await expect(page.getByTestId("onboarding-gate")).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "Re-import your key" }),
  ).toHaveCount(0);
});

test("local-owner locked recovery does not advertise key replacement", async ({
  page,
}) => {
  await installMockBridge(
    page,
    {
      identityLocked: true,
      localOwnerProfile: localOwnerProfile(TEST_IDENTITIES.alice.pubkey),
    },
    { skipOnboardingSeed: true },
  );
  await page.goto("/");

  await expect(page.getByTestId("keyring-locked")).toBeVisible();
  await expect(
    page.getByText("Only re-import the matching owner key", { exact: false }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Re-import the owner key" }).click();

  const wrongNsec = nsecEncode(hexToBytes(TEST_IDENTITIES.bob.privateKey));
  await page.getByTestId("nostr-import-nsec-input").fill(wrongNsec);
  await page.getByTestId("nostr-import-submit").click();
  await expect(page.getByTestId("nostr-import-feedback")).toContainText(
    "local-owner build requires the ratified #local-dev owner identity",
  );
  await expect(page.getByTestId("relaunch-required")).toHaveCount(0);
});

test("locked boot can re-import a key and requires relaunch", async ({
  page,
}) => {
  await installMockBridge(
    page,
    { identityLocked: true },
    { skipOnboardingSeed: true },
  );
  await page.goto("/");

  await expect(page.getByTestId("keyring-locked")).toBeVisible();
  page.on("dialog", (dialog) => dialog.accept());
  await page
    .getByRole("button", { name: "Re-import your key instead" })
    .click();

  const importedNsec = nsecEncode(hexToBytes(TEST_IDENTITIES.alice.privateKey));
  await page.getByTestId("nostr-import-nsec-input").fill(importedNsec);
  await expect(page.getByTestId("nostr-import-npub-preview")).toBeVisible();
  await page.getByTestId("nostr-import-submit").click();

  await expect(page.getByTestId("relaunch-required")).toBeVisible();
  await expect(page.getByTestId("keyring-locked")).toHaveCount(0);
});

test("locked screen relaunch button records the process-restart invoke", async ({
  page,
}) => {
  await installMockBridge(
    page,
    { identityLocked: true },
    { skipOnboardingSeed: true },
  );
  await page.goto("/");

  await expect(page.getByTestId("keyring-locked")).toBeVisible();
  await page.getByTestId("relaunch-app").click();

  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (
            window as Window & {
              __BUZZ_E2E_COMMAND_PAYLOADS__?: Array<{ command: string }>;
            }
          ).__BUZZ_E2E_COMMAND_PAYLOADS__?.some(
            (e) => e.command === "plugin:process|restart",
          ) ?? false,
      ),
    )
    .toBe(true);
});
