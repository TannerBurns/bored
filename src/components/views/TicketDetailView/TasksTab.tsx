import { EpicPanel } from '../../board/TicketModal/EpicPanel';
import { TaskList } from '../../board/TaskList';
import type { Ticket, Column, EpicProgress } from '../../../types';

interface EpicData {
  epicChildren: Ticket[];
  epicProgress: EpicProgress | null;
  parentEpic: Ticket | null;
  loadingEpic: boolean;
  availableTickets: Ticket[];
  selectedChildId: string;
  setSelectedChildId: (id: string) => void;
  isAddingChild: boolean;
  handleAddChild: () => Promise<void>;
  handleRemoveChild: (childId: string) => Promise<void>;
  handleMoveChild: (childIndex: number, direction: 'up' | 'down') => Promise<void>;
}

interface TasksTabProps {
  ticket: Ticket;
  columns: Column[];
  epicData: EpicData;
}

export function TasksTab({ ticket, columns, epicData }: TasksTabProps) {
  return (
    <div className="space-y-4">
      {/* Epic Panel (for epic tickets) */}
      <EpicPanel
        ticket={ticket}
        columns={columns}
        epicChildren={epicData.epicChildren}
        epicProgress={epicData.epicProgress}
        parentEpic={epicData.parentEpic}
        loadingEpic={epicData.loadingEpic}
        availableTickets={epicData.availableTickets}
        selectedChildId={epicData.selectedChildId}
        setSelectedChildId={epicData.setSelectedChildId}
        isAddingChild={epicData.isAddingChild}
        handleAddChild={epicData.handleAddChild}
        handleRemoveChild={epicData.handleRemoveChild}
        handleMoveChild={epicData.handleMoveChild}
      />

      {/* Task Queue (for regular tickets) */}
      {!ticket.isEpic && <TaskList ticketId={ticket.id} />}

      {/* Empty state when neither applies */}
      {!ticket.isEpic && !ticket.epicId && (
        <div className="text-center py-8 text-board-text-muted">
          <p className="text-sm">No tasks queued yet</p>
          <p className="text-xs mt-1">Tasks will appear here when added to this ticket</p>
        </div>
      )}
    </div>
  );
}
