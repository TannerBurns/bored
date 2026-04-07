import { useState, useEffect } from 'react';
import { getProjects, getWorkspaces } from '../../lib/tauri';
import type { Project, Workspace } from '../../types';

export type ScopeType = 'project' | 'workspace';

interface ScopeSelectorProps {
  value?: { type: ScopeType; id: string } | null;
  onChange: (scope: { type: ScopeType; id: string } | null) => void;
  disabled?: boolean;
  allowEmpty?: boolean;
  emptyLabel?: string;
  className?: string;
  /** When true, only show projects (no workspaces). Used for spec builder. */
  projectsOnly?: boolean;
}

export function ScopeSelector({
  value,
  onChange,
  disabled = false,
  allowEmpty = true,
  emptyLabel = 'No scope',
  className = '',
  projectsOnly = false,
}: ScopeSelectorProps) {
  const [projects, setProjects] = useState<Project[]>([]);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const [p, w] = await Promise.all([
          getProjects(),
          projectsOnly ? Promise.resolve([]) : getWorkspaces(),
        ]);
        if (!cancelled) {
          setProjects(p);
          setWorkspaces(w);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    load();
    return () => { cancelled = true; };
  }, [projectsOnly]);

  useEffect(() => {
    if (!allowEmpty && !value && !loading) {
      const first = projects[0] || workspaces[0];
      if (first) {
        const type: ScopeType = 'path' in first ? 'project' : 'workspace';
        onChange({ type, id: first.id });
      }
    }
  }, [allowEmpty, value, loading, projects, workspaces, onChange]);

  const encodedValue = value ? `${value.type}:${value.id}` : '';

  const handleChange = (raw: string) => {
    if (!raw) {
      onChange(null);
      return;
    }
    const [type, ...rest] = raw.split(':');
    const id = rest.join(':');
    onChange({ type: type as ScopeType, id });
  };

  return (
    <select
      value={encodedValue}
      onChange={(e) => handleChange(e.target.value)}
      disabled={disabled || loading}
      className={`w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border disabled:opacity-50 ${className}`}
    >
      {allowEmpty && <option value="">{emptyLabel}</option>}
      {projects.length > 0 && (
        <optgroup label="Projects">
          {projects.map((p) => (
            <option key={`project:${p.id}`} value={`project:${p.id}`}>
              {p.name}
            </option>
          ))}
        </optgroup>
      )}
      {workspaces.length > 0 && (
        <optgroup label="Workspaces">
          {workspaces.map((ws) => (
            <option key={`workspace:${ws.id}`} value={`workspace:${ws.id}`}>
              {ws.name} ({ws.projectIds.length} projects)
            </option>
          ))}
        </optgroup>
      )}
    </select>
  );
}

/** Helper to convert projectId/workspaceId to ScopeSelector value */
export function toScopeValue(
  projectId?: string | null,
  workspaceId?: string | null,
): { type: ScopeType; id: string } | null {
  if (workspaceId) return { type: 'workspace', id: workspaceId };
  if (projectId) return { type: 'project', id: projectId };
  return null;
}
