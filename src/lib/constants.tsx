import type { ReactNode } from 'react';
import type { Priority } from '../types';

export const PRIORITY_COLORS: Record<Priority, string> = {
  low: 'bg-blue-400',
  medium: 'bg-yellow-500',
  high: 'bg-orange-500',
  urgent: 'bg-red-500',
};

export const PRIORITY_BORDER_COLORS: Record<Priority, string> = {
  low: 'border-l-blue-400',
  medium: 'border-l-yellow-500',
  high: 'border-l-orange-500',
  urgent: 'border-l-red-500',
};

export const PRIORITY_RING_COLORS: Record<Priority, string> = {
  low: 'ring-blue-400/50',
  medium: 'ring-yellow-500/50',
  high: 'ring-orange-500/50',
  urgent: 'ring-red-500/50',
};

export const PRIORITY_RING_HOVER_COLORS: Record<Priority, string> = {
  low: 'hover:ring-2 hover:ring-blue-400/80',
  medium: 'hover:ring-2 hover:ring-yellow-500/80',
  high: 'hover:ring-2 hover:ring-orange-500/80',
  urgent: 'hover:ring-2 hover:ring-red-500/80',
};

export const PRIORITY_LABELS: Record<Priority, string> = {
  low: 'Low',
  medium: 'Medium',
  high: 'High',
  urgent: 'Urgent',
};

interface ColumnColorSet {
  bg: string;
  dot: string;
  glow: string;
}

const COLUMN_COLOR_MAP: Record<string, ColumnColorSet> = {
  'Backlog':       { bg: 'bg-board-text-muted',  dot: 'bg-board-text-muted',  glow: '' },
  'Ready':         { bg: 'bg-status-info',        dot: 'bg-status-info',        glow: '' },
  'In Progress':   { bg: 'bg-status-warning',     dot: 'bg-status-warning',     glow: 'glow-warning' },
  'Blocked':       { bg: 'bg-status-error',        dot: 'bg-status-error',        glow: 'glow-error' },
  'Review':        { bg: 'bg-purple-500',          dot: 'bg-purple-500',          glow: '' },
  'Done':          { bg: 'bg-status-success',      dot: 'bg-status-success',      glow: 'glow-success' },
};

const DEFAULT_COLUMN_COLORS: ColumnColorSet = {
  bg: 'bg-board-text-muted',
  dot: 'bg-board-text-muted',
  glow: '',
};

export function getColumnColors(columnName: string): ColumnColorSet {
  return COLUMN_COLOR_MAP[columnName] ?? DEFAULT_COLUMN_COLORS;
}

export function getColumnBg(columnName: string): string {
  return getColumnColors(columnName).bg;
}

export function getColumnGlow(columnName: string): string {
  return getColumnColors(columnName).glow;
}

interface NavItem {
  id: string;
  label: string;
  icon: ReactNode;
}

export const NAV_ITEMS: NavItem[] = [
  {
    id: 'dashboard',
    label: 'Dashboard',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M3 3v18h18" />
        <path d="M18.7 8l-5.1 5.2-2.8-2.7L7 14.3" />
      </svg>
    ),
  },
  { 
    id: 'boards', 
    label: 'Boards',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <rect x="3" y="3" width="7" height="7" />
        <rect x="14" y="3" width="7" height="7" />
        <rect x="3" y="14" width="7" height="7" />
        <rect x="14" y="14" width="7" height="7" />
      </svg>
    ),
  },
  { 
    id: 'specs', 
    label: 'Specs',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z" />
        <polyline points="14 2 14 8 20 8" />
        <line x1="16" y1="13" x2="8" y2="13" />
        <line x1="16" y1="17" x2="8" y2="17" />
        <line x1="10" y1="9" x2="8" y2="9" />
      </svg>
    ),
  },
  {
    id: 'validation',
    label: 'Validation',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
        <polyline points="22 4 12 14.01 9 11.01" />
      </svg>
    ),
  },
  { 
    id: 'agents', 
    label: 'Agents',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <rect x="3" y="11" width="18" height="10" rx="2" />
        <circle cx="12" cy="5" r="2" />
        <path d="M12 7v4" />
        <line x1="8" y1="16" x2="8" y2="16" />
        <line x1="16" y1="16" x2="16" y2="16" />
      </svg>
    ),
  },
  { 
    id: 'projects', 
    label: 'Projects',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
      </svg>
    ),
  },
];
