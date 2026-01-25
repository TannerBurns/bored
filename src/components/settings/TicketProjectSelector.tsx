import { useEffect, useState } from 'react';
import { getProjects, setTicketProject } from '../../lib/tauri';
import type { Project } from '../../types';

interface TicketProjectSelectorProps {
  ticketId: string;
  currentProjectId?: string;
  boardDefaultProjectId?: string;
  onChange?: (projectId: string | null) => void;
}

export function TicketProjectSelector({
  ticketId,
  currentProjectId,
  boardDefaultProjectId,
  onChange,
}: TicketProjectSelectorProps) {
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string>(
    currentProjectId || ''
  );
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadProjects();
  }, []);

  useEffect(() => {
    setSelectedProjectId(currentProjectId || '');
  }, [currentProjectId]);

  const loadProjects = async () => {
    try {
      setLoading(true);
      const data = await getProjects();
      setProjects(data);
      setError(null);
    } catch (e) {
      setError(`Failed to load projects: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const handleChange = async (projectId: string) => {
    const value = projectId === '' ? null : projectId;
    setSelectedProjectId(projectId);

    try {
      setSaving(true);
      await setTicketProject(ticketId, value);
      setError(null);
      onChange?.(value);
    } catch (e) {
      setError(`Failed to update ticket project: ${e}`);
      // Revert selection on error
      setSelectedProjectId(currentProjectId || '');
    } finally {
      setSaving(false);
    }
  };

  // Get the board default project for display
  const boardDefaultProject = projects.find(
    (p) => p.id === boardDefaultProjectId
  );

  if (loading) {
    return <div className="text-gray-400 text-sm">Loading projects...</div>;
  }

  return (
    <div>
      <label className="block text-sm text-gray-400 mb-1">
        Project Override
      </label>
      {error && <div className="text-red-400 text-xs mb-1">{error}</div>}
      <select
        value={selectedProjectId}
        onChange={(e) => handleChange(e.target.value)}
        disabled={saving}
        className="w-full px-3 py-2 bg-gray-700 rounded text-white border border-gray-600 focus:border-blue-500 focus:outline-none disabled:opacity-50"
      >
        <option value="">
          {boardDefaultProject
            ? `Use board default (${boardDefaultProject.name})`
            : 'Use board default (none set)'}
        </option>
        {projects.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name} ({p.path.split('/').pop()})
          </option>
        ))}
      </select>
      <p className="text-xs text-gray-500 mt-1">
        Override the board's default project for this ticket.
      </p>
    </div>
  );
}
