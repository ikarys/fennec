import { request } from "./request";
import type { SessionSummary } from "./sessions";

export interface Project {
  id: string;
  name: string;
  workdir: string;
  branch: string | null;
  remote: string | null;
  sessions: SessionSummary[];
}

export interface SlashCommand {
  name: string;
  description: string;
}

/** An agent: a mode of the session (`plan`, `builder`, …), or a sub-agent that workflow nodes run in a fresh context. */
export interface Agent {
  id: string;
  name: string;
  description: string;
  color: string | null;
  subagent: boolean;
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
