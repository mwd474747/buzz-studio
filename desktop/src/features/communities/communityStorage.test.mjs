import assert from "node:assert/strict";
import test from "node:test";

import {
  canonicalizeLocalOwnerCommunity,
  clearCommunityStorage,
  initFirstCommunity,
  isLocalOwnerCommunity,
  migrateLegacyCommunityStorage,
  shouldAutoConnectDefaultRelay,
} from "./communityStorage.ts";

function createMemoryStorage(initial = {}) {
  const values = new Map(Object.entries(initial));
  return {
    values,
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, String(value)),
    removeItem: (key) => values.delete(key),
    clear: () => values.clear(),
    key: (index) => Array.from(values.keys())[index] ?? null,
    get length() {
      return values.size;
    },
  };
}

test("migrateLegacyCommunityStorage promotes current Buzz workspace state", () => {
  const storage = createMemoryStorage({
    "buzz-workspaces": '[{"id":"current"}]',
    "buzz-active-workspace-id": "current",
  });

  migrateLegacyCommunityStorage(storage);

  assert.equal(storage.getItem("buzz-communities"), '[{"id":"current"}]');
  assert.equal(storage.getItem("buzz-active-community-id"), "current");
});

test("migrateLegacyCommunityStorage does not overwrite new community state", () => {
  const storage = createMemoryStorage({
    "buzz-communities": '[{"id":"new"}]',
    "buzz-active-community-id": "new",
    "buzz-workspaces": '[{"id":"old"}]',
    "buzz-active-workspace-id": "old",
  });

  migrateLegacyCommunityStorage(storage);

  assert.equal(storage.getItem("buzz-communities"), '[{"id":"new"}]');
  assert.equal(storage.getItem("buzz-active-community-id"), "new");
});

test("signed-build relay defaults auto-connect during first-run onboarding", () => {
  assert.equal(
    shouldAutoConnectDefaultRelay("wss://buzz.block.builderlab.xyz"),
    true,
  );
  assert.equal(shouldAutoConnectDefaultRelay("ws://localhost:3000"), false);
  assert.equal(
    shouldAutoConnectDefaultRelay("ws://localhost:3300", true),
    true,
  );
  assert.equal(shouldAutoConnectDefaultRelay("ws://127.0.0.1:3000"), false);
  assert.equal(shouldAutoConnectDefaultRelay("ws://[::1]:3000"), false);
  assert.equal(shouldAutoConnectDefaultRelay("ws://0.0.0.0:3000"), false);
  assert.equal(shouldAutoConnectDefaultRelay("http://localhost:3000"), false);
  assert.equal(
    shouldAutoConnectDefaultRelay("https://relay.example.com"),
    false,
  );
  assert.equal(shouldAutoConnectDefaultRelay("relay.example.com"), false);
  assert.equal(shouldAutoConnectDefaultRelay("not a valid relay"), false);
});

test("local-owner community requires the exact owner relay without agent settings", () => {
  const exact = {
    id: "local",
    name: "Local Dev",
    relayUrl: "ws://localhost:3300",
    pubkey: "owner",
    addedAt: "2026-09-01T00:00:00.000Z",
  };
  assert.equal(
    isLocalOwnerCommunity(exact, "ws://localhost:3300", "owner"),
    true,
  );
  assert.equal(
    isLocalOwnerCommunity(
      { ...exact, relayUrl: "ws://localhost:3000" },
      "ws://localhost:3300",
      "owner",
    ),
    false,
  );
  assert.equal(
    isLocalOwnerCommunity(
      { ...exact, reposDir: "/tmp/legacy" },
      "ws://localhost:3300",
      "owner",
    ),
    false,
  );
});

test("local-owner canonicalization replaces stale and legacy state with one clean pinned row", () => {
  const ownerPubkey = "a".repeat(64);
  const current = {
    id: "local-owner",
    name: "Old local name",
    relayUrl: "ws://localhost:3300",
    pubkey: ownerPubkey,
    addedAt: "2026-09-01T00:00:00.000Z",
    token: "stale-token",
    reposDir: "/tmp/stale-repos",
    nsec: `nsec1${"q".repeat(58)}`,
  };
  const storage = createMemoryStorage({
    "buzz-communities": JSON.stringify([
      current,
      {
        id: "stale-remote",
        name: "Stale remote",
        relayUrl: "wss://stale.invalid",
        pubkey: "b".repeat(64),
        nsec: `nsec1${"p".repeat(58)}`,
      },
    ]),
    "buzz-active-community-id": "stale-remote",
    "buzz-workspaces": JSON.stringify([
      { id: "legacy", nsec: `nsec1${"z".repeat(58)}` },
    ]),
    "buzz-active-workspace-id": "legacy",
  });

  const result = canonicalizeLocalOwnerCommunity(
    current,
    "ws://localhost:3300",
    ownerPubkey,
    storage,
  );

  assert.equal(result?.changed, true);
  assert.deepEqual(JSON.parse(storage.getItem("buzz-communities")), [
    {
      id: "local-owner",
      name: "Local Dev",
      relayUrl: "ws://localhost:3300",
      pubkey: ownerPubkey,
      addedAt: "2026-09-01T00:00:00.000Z",
    },
  ]);
  assert.equal(storage.getItem("buzz-active-community-id"), "local-owner");
  assert.equal(storage.getItem("buzz-workspaces"), null);
  assert.equal(storage.getItem("buzz-active-workspace-id"), null);
  assert.equal(storage.getItem("buzz-communities").includes("nsec"), false);
});

test("local-owner canonicalization rolls back when the active-id write fails", () => {
  const ownerPubkey = "a".repeat(64);
  const priorCommunities = JSON.stringify([
    {
      id: "existing",
      name: "Existing",
      relayUrl: "wss://existing.invalid",
      pubkey: "b".repeat(64),
      addedAt: "2026-08-31T00:00:00.000Z",
    },
  ]);
  const storage = createMemoryStorage({
    "buzz-communities": priorCommunities,
    "buzz-active-community-id": "existing",
    "buzz-workspaces": '[{"id":"legacy"}]',
    "buzz-active-workspace-id": "legacy",
  });
  let failedCanonicalActiveWrite = false;
  storage.setItem = (key, value) => {
    if (
      key === "buzz-active-community-id" &&
      value === "local-owner" &&
      !failedCanonicalActiveWrite
    ) {
      failedCanonicalActiveWrite = true;
      throw new Error("simulated storage failure");
    }
    storage.values.set(key, String(value));
  };

  const result = canonicalizeLocalOwnerCommunity(
    {
      id: "local-owner",
      name: "Local Dev",
      relayUrl: "ws://localhost:3300",
      pubkey: ownerPubkey,
      addedAt: "2026-09-01T00:00:00.000Z",
    },
    "ws://localhost:3300",
    ownerPubkey,
    storage,
  );

  assert.equal(result, null);
  assert.equal(storage.getItem("buzz-communities"), priorCommunities);
  assert.equal(storage.getItem("buzz-active-community-id"), "existing");
  assert.equal(storage.getItem("buzz-workspaces"), '[{"id":"legacy"}]');
  assert.equal(storage.getItem("buzz-active-workspace-id"), "legacy");
});

test("failed first-community write preserves existing community data", () => {
  const storage = createMemoryStorage({
    "buzz-communities": '[{"id":"existing"}]',
    "buzz-workspaces": '[{"id":"legacy"}]',
    "buzz-active-workspace-id": "legacy",
  });
  storage.setItem = (key, value) => {
    if (key === "buzz-communities") {
      throw new Error("QuotaExceededError");
    }
    storage.values.set(key, String(value));
  };
  globalThis.localStorage = storage;
  globalThis.window = { localStorage: storage };

  assert.equal(initFirstCommunity("wss://relay.example.com", "pubkey"), null);
  assert.equal(storage.getItem("buzz-communities"), '[{"id":"existing"}]');
  assert.equal(storage.getItem("buzz-active-community-id"), null);
  assert.equal(storage.getItem("buzz-workspaces"), '[{"id":"legacy"}]');
  assert.equal(storage.getItem("buzz-active-workspace-id"), "legacy");
});

test("clearCommunityStorage removes new and legacy state", () => {
  const storage = createMemoryStorage({
    "buzz-communities": "new",
    "buzz-active-community-id": "new",
    "buzz-workspaces": "old",
    "buzz-active-workspace-id": "old",
  });

  clearCommunityStorage(storage);
  migrateLegacyCommunityStorage(storage);

  assert.equal(storage.length, 0);
});
