import { index, rootRoute, route } from "@tanstack/virtual-file-routes";

/** Routes compiled into the governed local-owner interaction product. */
export const routes = rootRoute("root.tsx", [
  index("index.tsx"),
  route("/settings", "settings.tsx"),
  route("/messages/new", "messages.new.tsx"),
  route("/channels/$channelId", "channels.$channelId.tsx"),
  route(
    "/channels/$channelId/posts/$postId",
    "channels.$channelId.posts.$postId.tsx",
  ),
]);
