import assert from "node:assert/strict";
import test from "node:test";

import { isSettingsSectionAllowedForLocalOwnerPolicy } from "./settingsLocalOwner.ts";

test("ordinary desktop keeps Agents and Members while policy is unsettled", () => {
  for (const policy of ["loading", "inactive", "unavailable"]) {
    assert.equal(
      isSettingsSectionAllowedForLocalOwnerPolicy(policy, "agents"),
      true,
      policy,
    );
    assert.equal(
      isSettingsSectionAllowedForLocalOwnerPolicy(policy, "community-members"),
      true,
      policy,
    );
  }
});

test("active local-owner keeps the interaction surface and hides retired planes", () => {
  assert.equal(
    isSettingsSectionAllowedForLocalOwnerPolicy("active", "profile"),
    true,
  );
  assert.equal(
    isSettingsSectionAllowedForLocalOwnerPolicy("active", "appearance"),
    true,
  );
  assert.equal(
    isSettingsSectionAllowedForLocalOwnerPolicy("active", "agents"),
    false,
  );
  assert.equal(
    isSettingsSectionAllowedForLocalOwnerPolicy("active", "community-members"),
    false,
  );
  assert.equal(
    isSettingsSectionAllowedForLocalOwnerPolicy("active", "voice"),
    false,
  );
  assert.equal(
    isSettingsSectionAllowedForLocalOwnerPolicy("active", "local-archive"),
    false,
  );
  assert.equal(
    isSettingsSectionAllowedForLocalOwnerPolicy("active", "updates"),
    false,
  );
  assert.equal(
    isSettingsSectionAllowedForLocalOwnerPolicy("active", "mobile"),
    false,
  );
});
