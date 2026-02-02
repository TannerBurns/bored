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
  low: 'hover:ring-blue-400/80',
  medium: 'hover:ring-yellow-500/80',
  high: 'hover:ring-orange-500/80',
  urgent: 'hover:ring-red-500/80',
};

export const PRIORITY_LABELS: Record<Priority, string> = {
  low: 'Low',
  medium: 'Medium',
  high: 'High',
  urgent: 'Urgent',
};
