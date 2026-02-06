import type { Column, Project } from '../../../types';

export interface TicketEditFormProps {
  columns: Column[];
  projects: Project[];
  projectsLoading: boolean;
  // Edit state
  editColumnId: string;
  setEditColumnId: (id: string) => void;
  editPriority: 'low' | 'medium' | 'high' | 'urgent';
  setEditPriority: (priority: 'low' | 'medium' | 'high' | 'urgent') => void;
  editLabels: string;
  setEditLabels: (labels: string) => void;
  editProjectId: string;
  setEditProjectId: (id: string) => void;
  editModel: string;
  setEditModel: (model: string) => void;
  editBranchName: string;
  setEditBranchName: (branch: string) => void;
}

export function TicketEditForm({
  columns,
  projects,
  projectsLoading,
  editColumnId,
  setEditColumnId,
  editPriority,
  setEditPriority,
  editLabels,
  setEditLabels,
  editProjectId,
  setEditProjectId,
  editModel,
  setEditModel,
  editBranchName,
  setEditBranchName,
}: TicketEditFormProps) {
  return (
    <>
      {/* Column */}
      <div>
        <h3 className="text-sm font-medium text-board-text-muted mb-2">Column</h3>
        <select
          value={editColumnId}
          onChange={(e) => setEditColumnId(e.target.value)}
          className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
        >
          {columns.map((column) => (
            <option key={column.id} value={column.id}>
              {column.name}
            </option>
          ))}
        </select>
      </div>

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
        <input
          type="text"
          value={editLabels}
          onChange={(e) => setEditLabels(e.target.value)}
          placeholder="bug, frontend, urgent"
          className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text placeholder-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
        />
      </div>

      {/* Project */}
      <div>
        <h3 className="text-sm font-medium text-board-text-muted mb-2">Project</h3>
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
      </div>

      {/* AI Model */}
      <div>
        <h3 className="text-sm font-medium text-board-text-muted mb-2">AI Model</h3>
        <select
          value={editModel}
          onChange={(e) => setEditModel(e.target.value)}
          className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
        >
          <option value="">Default (Opus 4.6)</option>
          <option value="sonnet-4.5">Sonnet 4.5</option>
        </select>
        <p className="mt-1 text-xs text-board-text-muted">
          Select AI model for agent runs
        </p>
      </div>

      {/* Branch Name */}
      <div>
        <h3 className="text-sm font-medium text-board-text-muted mb-2">Branch Name</h3>
        <input
          type="text"
          value={editBranchName}
          onChange={(e) => setEditBranchName(e.target.value)}
          placeholder="feat/JIRA-123/add-feature"
          className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text placeholder-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border font-mono text-sm"
        />
        <p className="mt-1 text-xs text-board-text-muted">
          Leave empty for AI-generated branch name on first run
        </p>
      </div>
    </>
  );
}
