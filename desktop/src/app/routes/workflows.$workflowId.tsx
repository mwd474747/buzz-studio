import * as React from "react";
import { createFileRoute, Navigate } from "@tanstack/react-router";

import { usePreviewFeatureWarning } from "@/shared/features";
import { ViewLoadingFallback } from "@/shared/ui/ViewLoadingFallback";
import { useLocalOwnerPolicy } from "@/features/onboarding/useLocalOwnerPolicy";

export const Route = createFileRoute("/workflows/$workflowId")({
  component: WorkflowDetailRouteComponent,
});

const WorkflowsRouteScreen = React.lazy(async () => {
  const module = await import("./WorkflowsRouteScreen");
  return { default: module.WorkflowsRouteScreen };
});

function WorkflowDetailRouteComponent() {
  const localOwnerPolicy = useLocalOwnerPolicy();
  usePreviewFeatureWarning("workflows");
  const { workflowId } = Route.useParams();

  if (localOwnerPolicy === "loading") {
    return <ViewLoadingFallback kind="workflows" />;
  }
  if (localOwnerPolicy !== "inactive") {
    return <Navigate replace to="/" />;
  }

  return (
    <React.Suspense fallback={<ViewLoadingFallback kind="workflows" />}>
      <WorkflowsRouteScreen selectedWorkflowId={workflowId} />
    </React.Suspense>
  );
}
