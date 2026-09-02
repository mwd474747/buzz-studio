import * as React from "react";
import { createFileRoute, Navigate } from "@tanstack/react-router";

import { usePreviewFeatureWarning } from "@/shared/features";
import { ViewLoadingFallback } from "@/shared/ui/ViewLoadingFallback";
import { useLocalOwnerPolicy } from "@/features/onboarding/useLocalOwnerPolicy";

export const Route = createFileRoute("/workflows")({
  component: WorkflowsRouteComponent,
});

const WorkflowsRouteScreen = React.lazy(async () => {
  const module = await import("./WorkflowsRouteScreen");
  return { default: module.WorkflowsRouteScreen };
});

function WorkflowsRouteComponent() {
  const localOwnerPolicy = useLocalOwnerPolicy();
  usePreviewFeatureWarning("workflows");
  if (localOwnerPolicy === "loading") {
    return <ViewLoadingFallback kind="workflows" />;
  }
  if (localOwnerPolicy !== "inactive") {
    return <Navigate replace to="/" />;
  }
  return (
    <React.Suspense fallback={<ViewLoadingFallback kind="workflows" />}>
      <WorkflowsRouteScreen selectedWorkflowId={null} />
    </React.Suspense>
  );
}
