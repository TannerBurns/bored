import type { Ticket, Project } from '../../../types';

export interface TicketDetailsProps {
  ticket: Ticket;
  projects: Project[];
}

export function TicketDetails({ ticket, projects }: TicketDetailsProps) {
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

      {/* Project */}
      {ticket.projectId && (
        <div>
          <h3 className="text-sm font-medium text-board-text-muted mb-1">Project</h3>
          <code className="text-sm text-board-text-secondary bg-board-surface px-2 py-1 rounded">
            {projects.find(p => p.id === ticket.projectId)?.name || ticket.projectId}
          </code>
        </div>
      )}

      {/* Branch Name */}
      <div>
        <h3 className="text-sm font-medium text-board-text-muted mb-1">
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
