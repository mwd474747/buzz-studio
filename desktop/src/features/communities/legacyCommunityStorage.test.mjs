import assert from "node:assert/strict";
import test from "node:test";

import {
  applyLegacyCommunityStorage,
  migrateLegacyCommunityStorageBeforeRender,
} from "./legacyCommunityStorage.ts";
import { loadActiveCommunityId, loadCommunities } from "./communityStorage.ts";

function createMemoryStorage(initial = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem(key) {
      return values.has(key) ? values.get(key) : null;
    },
    setItem(key, value) {
      values.set(key, String(value));
    },
    removeItem(key) {
      values.delete(key);
    },
    clear() {
      values.clear();
    },
    key(index) {
      return Array.from(values.keys())[index] ?? null;
    },
    get length() {
      return values.size;
    },
  };
}

const legacyCommunities = JSON.stringify([
  {
    id: "legacy-community",
    name: "Existing relay",
    relayUrl: "wss://relay.example.com",
    addedAt: "2026-06-12T00:00:00.000Z",
  },
]);

const currentCommunities = JSON.stringify([
  {
    id: "current-community",
    name: "Current relay",
    relayUrl: "wss://current.example.com",
    addedAt: "2026-06-12T00:00:00.000Z",
  },
]);

const localhostCommunities = JSON.stringify([
  {
    id: "local-community",
    name: "Local Dev",
    relayUrl: "ws://localhost:3000",
    addedAt: "2026-06-12T00:00:00.000Z",
  },
]);

test("applyLegacyCommunityStorage seeds missing communities and active community", () => {
  const storage = createMemoryStorage();

  applyLegacyCommunityStorage(
    {
      workspaces: legacyCommunities,
      activeWorkspaceId: "legacy-community",
      onboardingCompletions: [],
    },
    storage,
  );

  assert.equal(storage.getItem("buzz-communities"), legacyCommunities);
  assert.equal(storage.getItem("buzz-active-community-id"), "legacy-community");
});

test("applyLegacyCommunityStorage preserves existing non-local Buzz communities", () => {
  const storage = createMemoryStorage({
    "buzz-communities": currentCommunities,
    "buzz-active-community-id": "current-community",
  });

  applyLegacyCommunityStorage(
    {
      workspaces: legacyCommunities,
      activeWorkspaceId: "legacy-community",
      onboardingCompletions: [],
    },
    storage,
  );

  assert.equal(storage.getItem("buzz-communities"), currentCommunities);
  assert.equal(
    storage.getItem("buzz-active-community-id"),
    "current-community",
  );
});

test("applyLegacyCommunityStorage replaces broken localhost first-run community", () => {
  const storage = createMemoryStorage({
    "buzz-communities": localhostCommunities,
    "buzz-active-community-id": "local-community",
  });

  applyLegacyCommunityStorage(
    {
      workspaces: legacyCommunities,
      activeWorkspaceId: "legacy-community",
      onboardingCompletions: [],
    },
    storage,
  );

  assert.equal(storage.getItem("buzz-communities"), legacyCommunities);
  assert.equal(storage.getItem("buzz-active-community-id"), "legacy-community");
});

test("applyLegacyCommunityStorage treats trailing-slash localhost as broken", () => {
  const storage = createMemoryStorage({
    "buzz-communities": JSON.stringify([
      {
        id: "local-community",
        name: "Local Dev",
        relayUrl: "ws://localhost:3000/",
        addedAt: "2026-06-12T00:00:00.000Z",
      },
    ]),
    "buzz-active-community-id": "local-community",
  });

  applyLegacyCommunityStorage(
    {
      workspaces: legacyCommunities,
      activeWorkspaceId: "legacy-community",
      onboardingCompletions: [],
    },
    storage,
  );

  assert.equal(storage.getItem("buzz-communities"), legacyCommunities);
  assert.equal(storage.getItem("buzz-active-community-id"), "legacy-community");
});

test("applyLegacyCommunityStorage migrates onboarding completion keys", () => {
  const storage = createMemoryStorage();

  applyLegacyCommunityStorage(
    {
      workspaces: null,
      activeWorkspaceId: null,
      onboardingCompletions: [{ pubkey: "abc123", value: "true" }],
    },
    storage,
  );

  assert.equal(storage.getItem("buzz-onboarding-complete.v1:abc123"), "true");
});

test("local-owner pre-render canonicalization removes secret-bearing storage before providers load", async () => {
  const legacySecret = JSON.stringify([
    { nsec: "nsec1mustneverenterthecurrentstore" },
  ]);
  const storage = createMemoryStorage({ "buzz-workspaces": legacySecret });
  let nativeReads = 0;

  await migrateLegacyCommunityStorageBeforeRender(
    storage,
    async () => ({
      profile: "local-owner",
      owner_pubkey: "a".repeat(64),
      relay_ws_url: "ws://localhost:3300",
    }),
    async () => {
      nativeReads += 1;
      return {
        workspaces: legacySecret,
        activeWorkspaceId: "legacy",
        onboardingCompletions: [],
      };
    },
  );

  assert.equal(nativeReads, 0);
  const communities = loadCommunities(storage);
  assert.equal(communities.length, 1);
  assert.equal(communities[0].pubkey, "a".repeat(64));
  assert.equal(communities[0].relayUrl, "ws://localhost:3300");
  assert.equal(loadActiveCommunityId(storage), communities[0].id);
  assert.equal(storage.getItem("buzz-workspaces"), null);
  assert.equal(storage.getItem("buzz-communities").includes("nsec"), false);
});

test("local-owner pre-render canonicalization preserves the exact pinned community identity", async () => {
  const ownerPubkey = "a".repeat(64);
  const addedAt = "2026-09-01T00:00:00.000Z";
  const storage = createMemoryStorage({
    "buzz-communities": JSON.stringify([
      {
        id: "stable-local-owner",
        name: "Old local name",
        relayUrl: "ws://localhost:3300",
        pubkey: ownerPubkey,
        addedAt,
        nsec: "nsec1mustberemoved",
      },
      { id: "stale", relayUrl: "wss://stale.invalid", nsec: "nsec1stale" },
    ]),
    "buzz-active-community-id": "stable-local-owner",
  });
  const profile = async () => ({
    profile: "local-owner",
    owner_pubkey: ownerPubkey,
    relay_ws_url: "ws://localhost:3300",
  });

  await migrateLegacyCommunityStorageBeforeRender(storage, profile);
  const first = storage.getItem("buzz-communities");
  await migrateLegacyCommunityStorageBeforeRender(storage, profile);

  assert.equal(storage.getItem("buzz-communities"), first);
  assert.deepEqual(JSON.parse(first), [
    {
      id: "stable-local-owner",
      name: "Local Dev",
      relayUrl: "ws://localhost:3300",
      pubkey: ownerPubkey,
      addedAt,
    },
  ]);
  assert.equal(
    storage.getItem("buzz-active-community-id"),
    "stable-local-owner",
  );
});

test("local-owner pre-render blocks provider mount when clean storage cannot commit", async () => {
  const storage = createMemoryStorage({
    "buzz-workspaces": '[{"nsec":"nsec1legacy"}]',
  });
  storage.setItem = () => {
    throw new Error("simulated storage failure");
  };

  await assert.rejects(
    migrateLegacyCommunityStorageBeforeRender(storage, async () => ({
      profile: "local-owner",
      owner_pubkey: "a".repeat(64),
      relay_ws_url: "ws://localhost:3300",
    })),
    /could not establish clean local-owner community storage/,
  );
  assert.equal(storage.getItem("buzz-communities"), null);
});

test("pre-render blocks provider mount when native policy resolution fails", async () => {
  const legacySecret = '[{"nsec":"nsec1mustremainunread"}]';
  const storage = createMemoryStorage({ "buzz-workspaces": legacySecret });
  let nativeReads = 0;

  await assert.rejects(
    migrateLegacyCommunityStorageBeforeRender(
      storage,
      async () => {
        throw new Error("simulated policy failure");
      },
      async () => {
        nativeReads += 1;
        return {
          workspaces: legacySecret,
          activeWorkspaceId: "legacy",
          onboardingCompletions: [],
        };
      },
    ),
    /simulated policy failure/,
  );
  assert.equal(nativeReads, 0);
  assert.equal(storage.getItem("buzz-communities"), null);
  assert.equal(storage.getItem("buzz-workspaces"), legacySecret);
});
