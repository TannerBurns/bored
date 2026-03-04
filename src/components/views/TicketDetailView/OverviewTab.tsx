import { DescriptionSection } from '../../board/TicketModal/DescriptionSection';
import { BlockedTicketBanner } from '../../board/TicketModal/BlockedTicketBanner';
import { PausedTicketBanner } from '../../board/TicketModal/PausedTicketBanner';
import { NextStepsPanel } from '../../board/TicketModal/NextStepsPanel';
import type { Ticket, Column, Comment, Task } from '../../../types';
import type { UseTicketEditReturn } from '../../board/TicketModal/hooks/useTicketEdit';
import type { UseAgentEventsReturn } from '../../board/TicketModal/hooks/useAgentEvents';

interface OverviewTabProps {
  ticket: Ticket;
  columns: Column[];
  comments: Comment[];
  tasks: Task[];
  editState: UseTicketEditReturn;
  agentEvents: UseAgentEventsReturn;
  onUpdate: (ticketId: string, updates: Partial<Ticket>) => Promise<void>;
  onOpenFullscreen: () => void;
  onNavigateToChat?: () => void;
  onBack: () => void;
}

export function OverviewTab({
  ticket,
  columns,
  comments,
  tasks,
  editState,
  agentEvents,
  onUpdate,
  onOpenFullscreen,
  onNavigateToChat,
  onBack,
}: OverviewTabProps) {
  return (
    <div className="space-y-4">
      {/* Next Steps (for completed tickets with branches) - shown first for visibility */}
      <NextStepsPanel
        ticket={ticket}
        columns={columns}
        onNavigateToChat={onNavigateToChat}
      />

      {/* Blocked ticket banner */}
      <BlockedTicketBanner
        ticket={ticket}
        columns={columns}
        comments={comments}
        tasks={tasks}
        onUpdate={onUpdate}
      />

      {/* Paused ticket banner */}
      <PausedTicketBanner
        ticket={ticket}
        isTicketPaused={agentEvents.isTicketPaused}
        isResuming={agentEvents.isResuming}
        handleResumeTicket={() => agentEvents.handleResumeTicket(onBack)}
      />

      {/* Description */}
      <DescriptionSection
        description={ticket.descriptionMd}
        isEditing={editState.isEditing}
        editDescription={editState.editDescription}
        setEditDescription={editState.setEditDescription}
        onOpenFullscreen={onOpenFullscreen}
        defaultExpanded
      />
    </div>
  );
}
