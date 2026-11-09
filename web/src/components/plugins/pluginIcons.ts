// A plugin picks its icon by name from fennec's set and never ships markup.
import {
  ActivityIcon,
  AlertIcon,
  BellIcon,
  BranchIcon,
  CheckIcon,
  ClockIcon,
  CpuIcon,
  EyeIcon,
  FileIcon,
  GlobeIcon,
  LayersIcon,
  ServerIcon,
  TerminalIcon,
  TokensIcon,
  PuzzleIcon,
} from '../ui/Icons';
import type { PluginIcon } from '../../lib/api/plugins';

export const PLUGIN_ICONS: Record<PluginIcon, typeof PuzzleIcon> = {
  activity: ActivityIcon,
  alert: AlertIcon,
  bell: BellIcon,
  branch: BranchIcon,
  check: CheckIcon,
  clock: ClockIcon,
  cpu: CpuIcon,
  eye: EyeIcon,
  file: FileIcon,
  globe: GlobeIcon,
  layers: LayersIcon,
  server: ServerIcon,
  terminal: TerminalIcon,
  tokens: TokensIcon,
  puzzle: PuzzleIcon,
};
