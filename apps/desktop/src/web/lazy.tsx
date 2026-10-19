import { lazy, Suspense, type ComponentProps, type ComponentType } from "react";
import { IconLoader2 as Loader } from "@tabler/icons-react";
import { ICON_SIZE } from "../components/ui/icon";

/**
 * Heavy panels are fetched when they are first shown (route-level code
 * splitting, P6): the rich note editor, the knowledge map, the document
 * viewer, the repository pages and the System area stay out of the first load.
 */
function lazily<Props extends object>(load: () => Promise<{ default: ComponentType<Props> }>) {
  const Component = lazy(load) as unknown as ComponentType<Props>;
  return function Lazy(props: Props) {
    return (
      <Suspense fallback={<div className="shared-loading" role="status"><Loader className="spin" size={ICON_SIZE.lg} /> Loading</div>}>
        <Component {...props} />
      </Suspense>
    );
  };
}

type Module<Name extends string, Source> = Source extends Record<Name, ComponentType<infer Props>> ? Props : never;

export const KnowledgeMapPanel = lazily<Module<"KnowledgeMapPanel", typeof import("./map/KnowledgeMapPanel")>>(
  () => import("./map/KnowledgeMapPanel").then((module) => ({ default: module.KnowledgeMapPanel })),
);
export const WorkspaceHealthPanel = lazily<Module<"WorkspaceHealthPanel", typeof import("../components/WorkspaceHealthPanel")>>(
  () => import("../components/WorkspaceHealthPanel").then((module) => ({ default: module.WorkspaceHealthPanel })),
);
export const ReviewQueuePanel = lazily<Module<"ReviewQueuePanel", typeof import("../components/ReviewQueuePanel")>>(
  () => import("../components/ReviewQueuePanel").then((module) => ({ default: module.ReviewQueuePanel })),
);
export const DocumentViewer = lazily<Module<"DocumentViewer", typeof import("../components/DocumentViewer")>>(
  () => import("../components/DocumentViewer").then((module) => ({ default: module.DocumentViewer })),
);
export const RichNoteEditor = lazily<Module<"RichNoteEditor", typeof import("../components/RichNoteEditor")>>(
  () => import("../components/RichNoteEditor").then((module) => ({ default: module.RichNoteEditor })),
);
export const RepositoriesPanel = lazily<Module<"RepositoriesPanel", typeof import("./repositories/RepositoriesPanel")>>(
  () => import("./repositories/RepositoriesPanel").then((module) => ({ default: module.RepositoriesPanel })),
);
export const RepositorySettings = lazily<Module<"RepositorySettings", typeof import("./repositories/RepositorySettings")>>(
  () => import("./repositories/RepositorySettings").then((module) => ({ default: module.RepositorySettings })),
);
export const RepositoryDetailView = lazily<Module<"RepositoryDetailView", typeof import("./repositories/RepositoryPage")>>(
  () => import("./repositories/RepositoryPage").then((module) => ({ default: module.RepositoryDetailView })),
);
export const SystemArea = lazily<ComponentProps<typeof import("./SystemArea").default>>(() => import("./SystemArea"));
