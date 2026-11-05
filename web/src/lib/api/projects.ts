import { request } from "./request";
import type { SessionSummary } from "./sessions";

export interface Project {
  id: string;
  name: string;
  workdir: string;
  /** The scratch project behind the chat: no folder, no git. */
  chat?: boolean;
  /** The workdir is inside fennec's own store: a session's worktree, not a project of the human's. */
  sessionWorktree?: boolean;
  /** Pinned by the human: the server lists favourites first. */
  favorite: boolean;
  branch: string | null;
  remote: string | null;
  /** Opens the project in VS Code (through the WSL remote under WSL). */
  editorUrl: string;
  sessions: SessionSummary[];
}

export interface SlashCommand {
  name: string;
  description: string;
}

/** The step of a project an agent serves; the server sends agents ordered by it. */
export type AgentPhase = 'start' | 'frame' | 'build' | 'understand' | 'check';

/** An agent: a mode of the session (`plan`, `builder`, …), or a sub-agent that workflow nodes run in a fresh context. */
export interface Agent {
  id: string;
  name: string;
  description: string;
  color: string | null;
  subagent: boolean;
  phase: AgentPhase | null;
  /** Where its `*.agent.md` came from: a default, the human's, or the project's. */
  origin: 'default' | 'user' | 'project';
}

export interface Folder {
  name: string;
  path: string;
  hidden: boolean;
  /** A git repository (has a `.git`). */
  repo: boolean;
}

export interface FolderListing {
  path: string;
  parent: string | null;
  folders: Folder[];
}

/** The folders inside `path` (the home directory when omitted). */
export function listFolders(path?: string): Promise<FolderListing> {
  const query = path ? `?path=${encodeURIComponent(path)}` : "";
  return request<FolderListing>(`/api/folders${query}`);
}

/** Create a new project folder `name` inside `path` (an empty git repo). */
export function createFolder(path: string, name: string): Promise<{ path: string }> {
  return request<{ path: string }>("/api/folders/create", {
    method: "POST",
    body: JSON.stringify({ path, name }),
  });
}

export function getProjects(): Promise<Project[]> {
  return request<Project[]>("/api/projects");
}

export function createProject(workdir: string): Promise<Project> {
  return request<Project>("/api/projects", {
    method: "POST",
    body: JSON.stringify({ workdir }),
  });
}

export interface DeleteProjectResult {
  deleted: boolean;
  folderDeleted: boolean;
  folderError: string | null;
}

/** Delete a project; `deleteFolder` also removes its source folder. */
export function deleteProject(id: string, deleteFolder: boolean): Promise<DeleteProjectResult> {
  return request<DeleteProjectResult>(
    `/api/projects/${encodeURIComponent(id)}/delete`,
    {
      method: "POST",
      body: JSON.stringify({ deleteFolder }),
    },
  );
}

/** Pin or unpin a project; the server lists favourites first. */
export function setProjectFavorite(id: string, favorite: boolean): Promise<void> {
  return request<void>(`/api/projects/${encodeURIComponent(id)}/favorite`, {
    method: "POST",
    body: JSON.stringify({ favorite }),
  });
}

/** One project row on the cross-project Backlog table: its counts and queue state. */
export interface BacklogProject {
  project: { id: string; name: string };
  yours: number;
  running: number;
  ready: number;
  drafts: number;
  done: number;
  queue: {
    running: boolean;
    autonomous: boolean;
    next: { id: string; text: string; title: string } | null;
  };
}

/** `GET /api/backlog`: one table row per project with its idea counts and queue controls. */
export interface BacklogView {
  total: { yours: number; running: number; ready: number; drafts: number; done: number; projects: number };
  projects: BacklogProject[];
}

export function getBacklog(): Promise<BacklogView> {
  return request<BacklogView>("/api/backlog");
}

export function listCommands(projectId: string): Promise<SlashCommand[]> {
  return request<SlashCommand[]>(
    `/api/projects/${encodeURIComponent(projectId)}/commands`,
  );
}

export function listAgents(projectId: string): Promise<Agent[]> {
  return request<Agent[]>(
    `/api/projects/${encodeURIComponent(projectId)}/agents`,
  );
}

/** An ADR file of the project's `docs/adr/` (verified: read from disk). */
export interface Adr {
  number: number;
  file: string;
  title: string;
  status: string | null;
  date: string | null;
  body: string;
}

/** A module of the project map (verified: read from the code's imports), placed by the server. */
export interface MapModule {
  id: string;
  /** The modules linked by dependencies it is drawn with; "" when it has none. */
  group: string;
  files: number;
  tests: number;
  entry: boolean;
  layer: number;
  row: number;
  /** The first paragraph of its head file's leading doc comment (its authors' words). */
  summary: string | null;
  sources: MapSourceNote[];
  /** The modules it imports, and those importing it, the most imports first. */
  uses: MapLink[];
  usedBy: MapLink[];
  /** Top-left corner of its box (260 × 46), placed by the server. */
  x: number;
  y: number;
}

export interface MapSourceNote {
  path: string;
  summary: string | null;
}

export interface MapLink {
  module: string;
  imports: number;
}

export interface MapPoint {
  x: number;
  y: number;
}

export interface MapDependency {
  from: string;
  to: string;
  imports: number;
  /** Breaks one of the human's module rules. */
  violation: boolean;
  /** The arrow as a finished SVG path: right angles, rounded corners, hops at crossings. */
  path: string;
  /** Where the import count is shown. */
  label: MapPoint;
}

export interface MapGroup {
  name: string;
  modules: number;
}

export interface ModuleMap {
  modules: MapModule[];
  dependencies: MapDependency[];
  /** In display order: the biggest first, "" (modules without dependencies) last. */
  groups: MapGroup[];
}

/** A dependency edge without layout data. */
export interface MapEdge {
  from: string;
  to: string;
}

/** A dependency that breaks a module rule declared in `.agents/architecture`. */
export interface RuleBreak {
  from: string;
  to: string;
  imports: number;
  rule: string;
}

/** Dependencies added or removed since `docs/map.md`. */
export interface MapChange {
  added: MapEdge[];
  removed: MapEdge[];
}

/** How the code's dependencies compare with the intended architecture. */
export interface ArchitectureDrift {
  rules: number;
  violations: RuleBreak[];
  sinceMap: MapChange | null;
}

/** The committed `docs/map.md` against the map of the code as it is. */
export type MapFileState = "missing" | "stale" | "current";

/** A project's documentation file, read from disk (verified). */
export interface DocFile {
  path: string;
  name: string;
  content: string;
}

export interface ProjectOverview {
  map: ModuleMap;
  mapFile: MapFileState;
  drift: ArchitectureDrift;
}

export function getProjectOverview(
  projectId: string,
): Promise<ProjectOverview> {
  return request<ProjectOverview>(
    `/api/projects/${encodeURIComponent(projectId)}/overview`,
  );
}

/** The project's documentation: its markdown files and its ADRs (verified). */
export function getProjectDocs(
  projectId: string,
): Promise<{ files: DocFile[]; adrs: Adr[] }> {
  return request<{ files: DocFile[]; adrs: Adr[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/docs`,
  );
}

export function writeMapFile(
  projectId: string,
): Promise<{ mapFile: MapFileState }> {
  return request<{ mapFile: MapFileState }>(
    `/api/projects/${encodeURIComponent(projectId)}/map-file`,
    {
      method: "POST",
      body: "{}",
    },
  );
}

// The project dashboard: display-ready tiles and recent sessions.
export type DashboardIcon = 'sessions' | 'ideas' | 'tokens' | 'time' | 'cost' | 'map' | 'doc';

export interface DashboardTile {
  id: string;
  icon: DashboardIcon;
  label: string;
  figure: string;
  detail: string;
  tone: 'accent' | 'info' | 'ok' | 'attention' | 'err' | 'neutral';
  active: boolean;
  href: string | null;
  title: string | null;
}

export interface RecentSession {
  id: string;
  title: string;
  updatedAt: number;
  running: boolean;
  awaiting: boolean;
}

export interface ProjectDashboard {
  tiles: DashboardTile[];
  recent: RecentSession[];
}

export function getProjectDashboard(
  projectId: string,
): Promise<ProjectDashboard> {
  return request<ProjectDashboard>(
    `/api/projects/${encodeURIComponent(projectId)}/dashboard`,
  );
}

/** The project's monthly budget and what it spent this month, display-ready. */
export interface ProjectBudgetView {
  limitUsd: number | null;
  label: string;
  figure: string;
  detail: string;
  tone: DashboardTile['tone'];
  active: boolean;
}

export function getProjectBudget(projectId: string): Promise<ProjectBudgetView> {
  return request<ProjectBudgetView>(
    `/api/projects/${encodeURIComponent(projectId)}/budget`,
  );
}

/** Set the project's monthly budget; `null` removes it. */
export function setProjectBudget(
  projectId: string,
  limitUsd: number | null,
): Promise<ProjectBudgetView> {
  return request<ProjectBudgetView>(
    `/api/projects/${encodeURIComponent(projectId)}/budget`,
    {
      method: "POST",
      body: JSON.stringify({ limitUsd }),
    },
  );
}

export type IndicatorId =
  | 'integrity'
  | 'sensitiveReview'
  | 'understanding'
  | 'architecture'
  | 'security'
  | 'cost'
  | 'transmission'
  | 'traceability'
  | 'reviewLoad';

export interface IndicatorView {
  id: IndicatorId;
  label: string;
  figure: string;
  detail: string;
  tone: DashboardTile['tone'];
}

/** The project's verified indicators, read from recorded facts. */
export function getProjectIndicators(projectId: string): Promise<IndicatorView[]> {
  return request<{ indicators: IndicatorView[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/indicators`,
  ).then((body) => body.indicators);
}
