import * as React from "react";
import { createFileRoute, Navigate } from "@tanstack/react-router";

import { usePreviewFeatureWarning } from "@/shared/features";
import { ViewLoadingFallback } from "@/shared/ui/ViewLoadingFallback";
import { useLocalOwnerPolicy } from "@/features/onboarding/useLocalOwnerPolicy";

const ProjectsScreen = React.lazy(async () => {
  const module = await import("@/features/projects/ui/ProjectsScreen");
  return { default: module.ProjectsScreen };
});

export const Route = createFileRoute("/projects")({
  component: ProjectsRouteComponent,
});

function ProjectsRouteComponent() {
  const localOwnerPolicy = useLocalOwnerPolicy();
  usePreviewFeatureWarning("projects");
  if (localOwnerPolicy === "loading") {
    return <ViewLoadingFallback kind="projects" />;
  }
  if (localOwnerPolicy !== "inactive") {
    return <Navigate replace to="/" />;
  }
  return (
    <React.Suspense fallback={<ViewLoadingFallback kind="projects" />}>
      <ProjectsScreen />
    </React.Suspense>
  );
}
