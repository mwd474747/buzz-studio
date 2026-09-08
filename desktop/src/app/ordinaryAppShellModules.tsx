import * as React from "react";

const isLocalOwnerBuild = import.meta.env.MODE === "local-owner";

export const OrdinaryAgentCockpitEffects = isLocalOwnerBuild
  ? null
  : React.lazy(async () => {
      const module = await import("@/app/OrdinaryAgentCockpitEffects");
      return { default: module.OrdinaryAgentCockpitEffects };
    });

export const OrdinaryAppShellBoundary = isLocalOwnerBuild
  ? null
  : React.lazy(async () => {
      const module = await import("@/app/OrdinaryAppShellBoundary");
      return { default: module.OrdinaryAppShellBoundary };
    });

export const OrdinaryAgentManagementDialogs = isLocalOwnerBuild
  ? null
  : React.lazy(async () => {
      const module = await import(
        "@/features/agents/ui/AgentManagementDialogs"
      );
      return { default: module.AgentManagementDialogs };
    });

export const OrdinaryRequestedAgentCreateDialogs = isLocalOwnerBuild
  ? null
  : React.lazy(async () => {
      const module = await import(
        "@/features/agents/ui/RequestedAgentCreateDialogs"
      );
      return { default: module.RequestedAgentCreateDialogs };
    });

export const requestAgentCreation = isLocalOwnerBuild
  ? undefined
  : () => {
      void import("@/features/agents/openCreateAgentEvent").then(
        ({ requestOpenCreateAgent }) => requestOpenCreateAgent(),
      );
    };
