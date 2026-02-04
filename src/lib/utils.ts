import { clsx, type ClassValue } from 'clsx';

export function cn(...inputs: ClassValue[]) {
  return clsx(inputs);
}

export function getTimeAgo(date: Date): string {
  const now = new Date();
  const seconds = Math.floor((now.getTime() - date.getTime()) / 1000);

  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

export function formatDuration(startedAt: Date, endedAt: Date): string {
  const seconds = Math.floor((endedAt.getTime() - startedAt.getTime()) / 1000);
  
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) return `${minutes}m ${remainingSeconds}s`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}h ${remainingMinutes}m`;
}

/**
 * Normalize dependsOn to always be an array of strings.
 * Handles old format (string | null) and new format (string[]).
 */
export function normalizeDependencies(dependsOn: string[] | string | null | undefined): string[] {
  if (!dependsOn) return [];
  if (Array.isArray(dependsOn)) return dependsOn.filter(d => d && d.length > 0);
  return [dependsOn];
}
