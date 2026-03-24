import { Input } from '../../common/Input';
import type { Project, Workspace } from '../../../types';

export interface TicketEditFormProps {
  projects: Project[];
  projectsLoading: boolean;
  workspaces: Workspace[];
  workspacesLoading: boolean;
  // Edit state
  editPriority: 'low' | 'medium' | 'high' | 'urgent';
  setEditPriority: (priority: 'low' | 'medium' | 'high' | 'urgent') => void;
  editLabels: string;
  setEditLabels: (labels: string) => void;
  editProjectId: string;
  setEditProjectId: (id: string) => void;
  editWorkspaceId: string;
  setEditWorkspaceId: (id: string) => void;
  editBranchName: string;
  setEditBranchName: (branch: string) => void;
}

export function TicketEditForm({
  projects,
  projectsLoading,
  workspaces,
  workspacesLoading,
  editPriority,
  setEditPriority,
  editLabels,
  setEditLabels,
  editProjectId,
  setEditProjectId,
  editWorkspaceId,
  setEditWorkspaceId,
  editBranchName,
  setEditBranchName,
}: TicketEditFormProps) {
  const scopeMode = editWorkspaceId ? 'workspace' : 'project';

  const handleScopeModeChange = (mode: 'project' | 'workspace') => {
    if (mode === 'project') {
      setEditWorkspaceId('');
    } else {
      setEditProjectId('');
    }
  };

  return (
    <>
      {/* Priority */}
      <div>
        <h3 className="text-sm font-medium text-board-text-muted mb-2">Priority</h3>
        <select
          value={editPriority}
          onChange={(e) => setEditPriority(e.target.value as 'low' | 'medium' | 'high' | 'urgent')}
          className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
        >
          <option value="low">Low</option>
          <option value="medium">Medium</option>
          <option value="high">High</option>
          <option value="urgent">Urgent</option>
        </select>
      </div>

      {/* Labels */}
      <div>
        <h3 className="text-sm font-medium text-board-text-muted mb-2">Labels (comma-separated)</h3>
        <Input
          type="text"
          value={editLabels}
          onChange={(e) => setEditLabels(e.target.value)}
          placeholder="bug, frontend, urgent"
        />
      </div>

      {/* Scope toggle */}
      <div>
        <h3 className="text-sm font-medium text-board-text-muted mb-2">Scope</h3>
        <div className="flex gap-1 mb-2">
          <button
            type="button"
            onClick={() => handleScopeModeChange('project')}
            className={`px-3 py-1.5 text-xs rounded-lg border transition-colors ${
              scopeMode === 'project'
                ? 'bg-board-accent text-white border-board-accent'
                : 'bg-board-surface-raised text-board-text-muted border-board-border hover:bg-board-card-hover'
            }`}
          >
            Single Project
          </button>
          <button
            type="button"
            onClick={() => handleScopeModeChange('workspace')}
            disabled={workspaces.length === 0}
            className={`px-3 py-1.5 text-xs rounded-lg border transition-colors ${
              scopeMode === 'workspace'
                ? 'bg-board-accent text-white border-board-accent'
                : 'bg-board-surface-raised text-board-text-muted border-board-border hover:bg-board-card-hover'
            } disabled:opacity-40 disabled:cursor-not-allowed`}
          >
            Workspace
          </button>
        </div>

        {scopeMode === 'project' ? (
          <select
            value={editProjectId}
            onChange={(e) => setEditProjectId(e.target.value)}
            disabled={projectsLoading}
            className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border disabled:opacity-50"
          >
            <option value="">No project</option>
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name}
              </option>
            ))}
          </select>
        ) : (
          <select
            value={editWorkspaceId}
            onChange={(e) => setEditWorkspaceId(e.target.value)}
            disabled={workspacesLoading}
            className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border disabled:opacity-50"
          >
            <option value="">Select workspace</option>
            {workspaces.map((ws) => (
              <option key={ws.id} value={ws.id}>
                {ws.name} ({ws.projectIds.length} projects)
              </option>
            ))}
          </select>
        )}
      </div>

      {/* Branch Name */}
      <div>
        <h3 className="text-sm font-medium text-board-text-muted mb-2">Branch Name</h3>
        <Input
          type="text"
          value={editBranchName}
          onChange={(e) => setEditBranchName(e.target.value)}
          placeholder="feat/JIRA-123/add-feature"
          className="font-mono text-sm"
        />
        <p className="mt-1 text-xs text-board-text-muted">
          Leave empty for AI-generated branch name on first run
        </p>
      </div>
    </>
  );
}
