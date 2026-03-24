import type { Project, Workspace } from '../../types';

interface CreateWorkspaceFormProps {
  projects: Project[];
  wsName: string;
  setWsName: (v: string) => void;
  selectedProjectIds: Set<string>;
  onToggleProject: (id: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
  saving: boolean;
}

export function CreateWorkspaceForm({
  projects,
  wsName,
  setWsName,
  selectedProjectIds,
  onToggleProject,
  onSubmit,
  onCancel,
  saving,
}: CreateWorkspaceFormProps) {
  return (
    <div className="glass rounded-lg p-3 space-y-2">
      <div className="text-xs font-medium text-purple-400">Create Workspace</div>
      <div>
        <label className="block text-xs text-board-text-secondary mb-1">Workspace Name</label>
        <input
          type="text"
          value={wsName}
          onChange={(e) => setWsName(e.target.value)}
          placeholder="My Workspace"
          className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg text-sm text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-1 focus:ring-board-accent/20"
        />
      </div>
      <ProjectCheckboxList
        projects={projects}
        selectedIds={selectedProjectIds}
        onToggle={onToggleProject}
        emptyMessage="No projects available. Add projects first."
      />
      {selectedProjectIds.size > 0 && selectedProjectIds.size < 2 && (
        <p className="text-xs text-status-warning mt-1">Select at least 2 projects</p>
      )}
      <div className="flex justify-end gap-2 pt-1">
        <button onClick={onCancel} disabled={saving} className="px-2 py-1 text-xs text-board-text-muted hover:text-board-text transition-colors disabled:opacity-50">
          Cancel
        </button>
        <button
          onClick={onSubmit}
          disabled={!wsName.trim() || selectedProjectIds.size < 2 || saving}
          className="px-2 py-1 text-xs bg-purple-600 text-white rounded-lg hover:bg-purple-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {saving ? 'Creating...' : 'Create Workspace'}
        </button>
      </div>
    </div>
  );
}

interface EditWorkspaceFormProps {
  workspace: Workspace;
  projects: Project[];
  editName: string;
  setEditName: (v: string) => void;
  editProjectIds: Set<string>;
  onToggleProject: (id: string) => void;
  onSave: () => void;
  onCancel: () => void;
  saving: boolean;
}

export function EditWorkspaceForm({
  projects,
  editName,
  setEditName,
  editProjectIds,
  onToggleProject,
  onSave,
  onCancel,
  saving,
}: EditWorkspaceFormProps) {
  return (
    <div className="glass rounded-lg p-3 space-y-2 border border-purple-500/30">
      <div className="text-xs font-medium text-purple-400">Edit Workspace</div>
      <div>
        <label className="block text-xs text-board-text-secondary mb-1">Workspace Name</label>
        <input
          type="text"
          value={editName}
          onChange={(e) => setEditName(e.target.value)}
          className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg text-sm text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-1 focus:ring-board-accent/20"
        />
      </div>
      <div>
        <label className="block text-xs text-board-text-secondary mb-1">Member Projects</label>
        <ProjectCheckboxList
          projects={projects}
          selectedIds={editProjectIds}
          onToggle={onToggleProject}
        />
      </div>
      <div className="flex justify-end gap-2 pt-1">
        <button onClick={onCancel} disabled={saving} className="px-2 py-1 text-xs text-board-text-muted hover:text-board-text transition-colors disabled:opacity-50">
          Cancel
        </button>
        <button
          onClick={onSave}
          disabled={!editName.trim() || editProjectIds.size < 2 || saving}
          className="px-2 py-1 text-xs bg-purple-600 text-white rounded-lg hover:bg-purple-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {saving ? 'Saving...' : 'Save Workspace'}
        </button>
      </div>
    </div>
  );
}

interface ProjectCheckboxListProps {
  projects: Project[];
  selectedIds: Set<string>;
  onToggle: (id: string) => void;
  emptyMessage?: string;
}

function ProjectCheckboxList({ projects, selectedIds, onToggle, emptyMessage }: ProjectCheckboxListProps) {
  if (projects.length === 0 && emptyMessage) {
    return <p className="text-xs text-board-text-muted">{emptyMessage}</p>;
  }

  return (
    <div className="space-y-1 max-h-40 overflow-auto">
      {projects.map((p) => (
        <label
          key={p.id}
          className="flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-board-card-hover cursor-pointer transition-colors"
        >
          <input
            type="checkbox"
            checked={selectedIds.has(p.id)}
            onChange={() => onToggle(p.id)}
            className="rounded border-board-border text-board-accent focus:ring-board-accent"
          />
          <span className="text-sm text-board-text">{p.name}</span>
          <span className="text-xs text-board-text-muted font-mono truncate ml-auto">{p.path}</span>
        </label>
      ))}
    </div>
  );
}
