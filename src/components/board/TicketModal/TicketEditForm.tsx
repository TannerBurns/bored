import { Input } from '../../common/Input';
import { ScopeSelector, toScopeValue } from '../../common/ScopeSelector';

export interface TicketEditFormProps {
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

      {/* Scope */}
      <div>
        <h3 className="text-sm font-medium text-board-text-muted mb-2">Scope</h3>
        <ScopeSelector
          value={toScopeValue(editProjectId, editWorkspaceId)}
          onChange={(scope) => {
            if (!scope) {
              setEditProjectId('');
              setEditWorkspaceId('');
            } else if (scope.type === 'project') {
              setEditProjectId(scope.id);
              setEditWorkspaceId('');
            } else {
              setEditWorkspaceId(scope.id);
              setEditProjectId('');
            }
          }}
        />
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
