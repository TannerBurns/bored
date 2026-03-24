import type { Ticket, Project, Workspace } from '../../../types';

export interface TicketDetailsProps {
  ticket: Ticket;
  projects: Project[];
  workspaces: Workspace[];
}

export function TicketDetails({ ticket, projects, workspaces }: TicketDetailsProps) {
  const scopeName = ticket.projectId
    ? projects.find(p => p.id === ticket.projectId)?.name || ticket.projectId
    : ticket.workspaceId
      ? workspaces.find(w => w.id === ticket.workspaceId)?.name || ticket.workspaceId
      : null;
  const scopeLabel = ticket.projectId ? 'Project' : ticket.workspaceId ? 'Workspace' : null;

  return (
    <>
      {/* Labels */}
      {ticket.labels.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {ticket.labels.map((label) => (
            <span
              key={label}
              className="px-2 py-1 text-sm bg-board-surface rounded-full text-board-text-secondary"
            >
              {label}
            </span>
          ))}
        </div>
      )}

      {/* Scope (Project or Workspace) */}
      {scopeName && scopeLabel && (
        <div>
          <h3 className="text-base font-semibold text-board-text mb-1">{scopeLabel}</h3>
          <code className="text-sm text-board-text-secondary bg-board-surface px-2 py-1 rounded">
            {scopeName}
          </code>
        </div>
      )}

      {/* Branch Name */}
      <div>
        <h3 className="text-base font-semibold text-board-text mb-1">
          Branch Name
        </h3>
        {ticket.branchName ? (
          <code className="text-sm text-board-text-secondary bg-board-surface px-2 py-1 rounded font-mono">
            {ticket.branchName}
          </code>
        ) : (
          <span className="text-sm text-board-text-muted italic">
            Not set (will be AI-generated on first run)
          </span>
        )}
      </div>
    </>
  );
}
