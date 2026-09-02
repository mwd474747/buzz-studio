import assert from "node:assert/strict";
import test from "node:test";

class ElementShim {
  constructor() {
    this.children = [];
    this.childNodes = [];
    this.nodeType = 1;
    this.nodeName = "DIV";
    this.tagName = "DIV";
    this.namespaceURI = "http://www.w3.org/1999/xhtml";
  }
  get ownerDocument() {
    return globalThis.document;
  }
  addEventListener() {}
  removeEventListener() {}
  appendChild(child) {
    this.children.push(child);
    this.childNodes.push(child);
    return child;
  }
  removeChild(child) {
    this.children = this.children.filter((current) => current !== child);
    this.childNodes = this.childNodes.filter((current) => current !== child);
    return child;
  }
  insertBefore(child) {
    return this.appendChild(child);
  }
  contains(target) {
    return this === target;
  }
}

globalThis.document = {
  activeElement: null,
  addEventListener() {},
  createElement: () => new ElementShim(),
  get defaultView() {
    return globalThis.window;
  },
  nodeType: 9,
  removeEventListener() {},
};
Object.defineProperty(globalThis, "window", {
  configurable: true,
  value: {
    addEventListener() {},
    document: globalThis.document,
    event: undefined,
    HTMLIFrameElement: ElementShim,
    location: { href: "http://localhost/" },
    removeEventListener() {},
  },
});
globalThis.HTMLElement = ElementShim;
globalThis.Node = ElementShim;
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

import React, { act } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { useMachineOnboardingState } from "./machineOnboarding.ts";

function createMemoryStorage() {
  const values = new Map();
  return {
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

const OWNER_PUBKEY = "a".repeat(64);

async function mountMachineOnboarding(initialPolicy, identityOverrides = {}) {
  window.localStorage = createMemoryStorage();
  let latest;
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  queryClient.setQueryData(["identity"], {
    pubkey: OWNER_PUBKEY,
    displayName: "Local owner",
    lost: false,
    locked: false,
    resetFailed: false,
    ...identityOverrides,
  });

  function Harness({ localOwnerPolicy }) {
    latest = useMachineOnboardingState({
      activeCommunityPubkey: OWNER_PUBKEY,
      isSharedIdentity: false,
      localOwnerPolicy,
    });
    return null;
  }

  const root = createRoot(new ElementShim());
  const render = async (localOwnerPolicy) => {
    await act(async () => {
      root.render(
        React.createElement(
          QueryClientProvider,
          { client: queryClient },
          React.createElement(Harness, { localOwnerPolicy }),
        ),
      );
      await Promise.resolve();
    });
  };

  await render(initialPolicy);
  return {
    getStage: () => latest.stage,
    render,
    setIdentityError: async () => {
      await act(async () => {
        const query = queryClient.getQueryCache().find({
          queryKey: ["identity"],
        });
        query.setState({
          ...query.state,
          data: undefined,
          error: new Error("simulated identity failure"),
          fetchStatus: "idle",
          status: "error",
        });
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
    },
    unmount: async () => {
      await act(async () => root.unmount());
      queryClient.clear();
    },
  };
}

test("resolved identity leaves blocking when local-owner policy changes from loading to inactive", async () => {
  const mounted = await mountMachineOnboarding("loading");
  assert.equal(mounted.getStage(), "blocking");

  await mounted.render("inactive");
  assert.equal(mounted.getStage(), "ready");

  await mounted.unmount();
});

test("active exact local owner is ready without generic onboarding completion", async () => {
  const mounted = await mountMachineOnboarding("active");
  assert.equal(mounted.getStage(), "ready");

  await mounted.unmount();
});

test("active local owner stays blocked when identity resolution fails", async () => {
  const mounted = await mountMachineOnboarding("active");
  assert.equal(mounted.getStage(), "ready");

  await mounted.setIdentityError();
  assert.equal(mounted.getStage(), "blocking");

  await mounted.unmount();
});

test("native relaunch latch survives a fresh onboarding mount", async () => {
  const mounted = await mountMachineOnboarding("active", {
    relaunchRequired: true,
  });
  assert.equal(mounted.getStage(), "relaunch-required");

  await mounted.unmount();
});
