import { cn } from '../../../lib/utils';
import type { Ticket as TicketType, Column, EpicProgress } from '../../../types';

export interface EpicPanelProps {
  ticket: TicketType;
  columns: Column[];
  epicChildren: TicketType[];
  epicProgress: EpicProgress | null;
  parentEpic: TicketType | null;
  loadingEpic: boolean;
  availableTickets: TicketType[];
  selectedChildId: string;
  setSelectedChildId: (id: string) => void;
  isAddingChild: boolean;
  handleAddChild: () => Promise<void>;
  handleRemoveChild: (childId: string) => Promise<void>;
  handleMoveChild: (childIndex: number, direction: 'up' | 'down') => Promise<void>;
}

export function EpicPanel({
  ticket,
  columns,
  epicChildren,
  epicProgress,
  parentEpic,
  loadingEpic,
  availableTickets,
  selectedChildId,
  setSelectedChildId,
  isAddingChild,
  handleAddChild,
  handleRemoveChild,
  handleMoveChild,
}: EpicPanelProps) {
  if (!ticket.isEpic && !ticket.epicId) {
    return null;
  }

  return (
    <div>
      <h3 className="text-sm font-medium text-board-text-muted mb-2 flex items-center gap-2">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="text-purple-400"
        >
          <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
        </svg>
        {ticket.isEpic ? 'Epic Children' : 'Parent Epic'}
      </h3>
      
      {loadingEpic ? (
        <div className="text-sm text-board-text-muted">Loading...</div>
      ) : ticket.isEpic ? (
        <EpicChildrenView
          columns={columns}
          epicChildren={epicChildren}
          epicProgress={epicProgress}
          availableTickets={availableTickets}
          selectedChildId={selectedChildId}
          setSelectedChildId={setSelectedChildId}
          isAddingChild={isAddingChild}
          handleAddChild={handleAddChild}
          handleRemoveChild={handleRemoveChild}
          handleMoveChild={handleMoveChild}
        />
      ) : parentEpic ? (
        <ParentEpicView parentEpic={parentEpic} orderInEpic={ticket.orderInEpic} />
      ) : (
        <div className="bg-board-surface rounded-lg p-3">
          <p className="text-sm text-board-text-muted">Parent epic not found</p>
        </div>
      )}
    </div>
  );
}

interface EpicChildrenViewProps {
  columns: Column[];
  epicChildren: TicketType[];
  epicProgress: EpicProgress | null;
  availableTickets: TicketType[];
  selectedChildId: string;
  setSelectedChildId: (id: string) => void;
  isAddingChild: boolean;
  handleAddChild: () => Promise<void>;
  handleRemoveChild: (childId: string) => Promise<void>;
  handleMoveChild: (childIndex: number, direction: 'up' | 'down') => Promise<void>;
}

function EpicChildrenView({
  columns,
  epicChildren,
  epicProgress,
  availableTickets,
  selectedChildId,
  setSelectedChildId,
  isAddingChild,
  handleAddChild,
  handleRemoveChild,
  handleMoveChild,
}: EpicChildrenViewProps) {
  return (
    <div className="bg-board-surface rounded-lg p-3">
      {epicProgress && epicProgress.total > 0 ? (
        <>
          {/* Progress bar */}
          <div className="mb-3">
            <div className="flex justify-between text-xs text-board-text-muted mb-1">
              <span>{epicProgress.done} of {epicProgress.total} done</span>
              <span>{Math.round((epicProgress.done / epicProgress.total) * 100)}%</span>
            </div>
            <div className="h-2 bg-board-surface-raised rounded-full overflow-hidden">
              <div 
                className="h-full bg-status-success rounded-full transition-all"
                style={{ width: `${(epicProgress.done / epicProgress.total) * 100}%` }}
              />
            </div>
          </div>
          
          {/* Status breakdown */}
          <div className="grid grid-cols-3 gap-2 text-xs mb-3">
            {epicProgress.backlog > 0 && (
              <div className="text-center p-1.5 bg-board-surface-raised rounded">
                <div className="font-medium text-board-text">{epicProgress.backlog}</div>
                <div className="text-board-text-muted">Backlog</div>
              </div>
            )}
            {epicProgress.ready > 0 && (
              <div className="text-center p-1.5 bg-board-surface-raised rounded">
                <div className="font-medium text-board-text">{epicProgress.ready}</div>
                <div className="text-board-text-muted">Ready</div>
              </div>
            )}
            {epicProgress.inProgress > 0 && (
              <div className="text-center p-1.5 bg-status-warning/10 rounded border border-status-warning/30">
                <div className="font-medium text-status-warning">{epicProgress.inProgress}</div>
                <div className="text-board-text-muted">In Progress</div>
              </div>
            )}
            {epicProgress.blocked > 0 && (
              <div className="text-center p-1.5 bg-status-error/10 rounded border border-status-error/30">
                <div className="font-medium text-status-error">{epicProgress.blocked}</div>
                <div className="text-board-text-muted">Blocked</div>
              </div>
            )}
            {epicProgress.review > 0 && (
              <div className="text-center p-1.5 bg-board-surface-raised rounded">
                <div className="font-medium text-board-text">{epicProgress.review}</div>
                <div className="text-board-text-muted">Review</div>
              </div>
            )}
            {epicProgress.done > 0 && (
              <div className="text-center p-1.5 bg-status-success/10 rounded border border-status-success/30">
                <div className="font-medium text-status-success">{epicProgress.done}</div>
                <div className="text-board-text-muted">Done</div>
              </div>
            )}
          </div>
          
          {/* Children list */}
          <div className="space-y-1 max-h-40 overflow-y-auto">
            {epicChildren.map((child, index) => (
              <div 
                key={child.id}
                className="flex items-center gap-2 text-sm p-2 bg-board-surface-raised rounded group"
              >
                {/* Reorder buttons */}
                <div className="flex flex-col opacity-0 group-hover:opacity-100 transition-opacity">
                  <button
                    onClick={() => handleMoveChild(index, 'up')}
                    disabled={index === 0}
                    className="p-0.5 text-board-text-muted hover:text-board-text disabled:opacity-30 disabled:cursor-not-allowed"
                    title="Move up"
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <polyline points="18 15 12 9 6 15" />
                    </svg>
                  </button>
                  <button
                    onClick={() => handleMoveChild(index, 'down')}
                    disabled={index === epicChildren.length - 1}
                    className="p-0.5 text-board-text-muted hover:text-board-text disabled:opacity-30 disabled:cursor-not-allowed"
                    title="Move down"
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <polyline points="6 9 12 15 18 9" />
                    </svg>
                  </button>
                </div>
                <span className="text-board-text-muted w-5 text-center">{index + 1}</span>
                <span className="flex-1 truncate text-board-text-secondary">{child.title}</span>
                <span className={cn(
                  'text-xs px-1.5 py-0.5 rounded',
                  child.lockedByRunId ? 'bg-status-warning/20 text-status-warning' :
                  columns.find(c => c.id === child.columnId)?.name === 'Done' ? 'bg-status-success/20 text-status-success' :
                  columns.find(c => c.id === child.columnId)?.name === 'Blocked' ? 'bg-status-error/20 text-status-error' :
                  'bg-board-surface text-board-text-muted'
                )}>
                  {columns.find(c => c.id === child.columnId)?.name || 'Unknown'}
                </span>
                <button
                  onClick={() => handleRemoveChild(child.id)}
                  className="opacity-0 group-hover:opacity-100 p-1 text-board-text-muted hover:text-status-error transition-all"
                  title="Remove from epic"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <line x1="18" y1="6" x2="6" y2="18" />
                    <line x1="6" y1="6" x2="18" y2="18" />
                  </svg>
                </button>
              </div>
            ))}
          </div>
        </>
      ) : null}
      
      {/* Add child section */}
      {availableTickets.length > 0 && (
        <div className={cn("flex gap-2 items-center", epicProgress && epicProgress.total > 0 && "mt-3 pt-3 border-t border-board-border")}>
          <select
            value={selectedChildId}
            onChange={(e) => setSelectedChildId(e.target.value)}
            className="flex-1 px-2 py-1.5 text-sm bg-board-surface-raised rounded border border-board-border text-board-text focus:outline-none focus:ring-1 focus:ring-purple-500"
          >
            <option value="">Select ticket to add...</option>
            {availableTickets.map((t) => (
              <option key={t.id} value={t.id}>
                {t.title}
              </option>
            ))}
          </select>
          <button
            onClick={handleAddChild}
            disabled={!selectedChildId || isAddingChild}
            className="px-3 py-1.5 text-sm bg-purple-600 hover:bg-purple-700 text-white rounded disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {isAddingChild ? 'Adding...' : 'Add'}
          </button>
        </div>
      )}
      
      {!epicProgress?.total && availableTickets.length === 0 && (
        <p className="text-sm text-board-text-muted">No children yet. Create tickets in the Backlog or Ready column to add them to this epic.</p>
      )}
    </div>
  );
}

interface ParentEpicViewProps {
  parentEpic: TicketType;
  orderInEpic?: number;
}

function ParentEpicView({ parentEpic, orderInEpic }: ParentEpicViewProps) {
  return (
    <div className="bg-board-surface rounded-lg p-3">
      <div className="flex items-center gap-2">
        <span className="inline-flex items-center gap-1 text-xs px-1.5 py-0.5 bg-purple-500/20 text-purple-300 rounded font-medium">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="10"
            height="10"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
          </svg>
          Epic
        </span>
        <span className="text-sm text-board-text-secondary">{parentEpic.title}</span>
      </div>
      <div className="text-xs text-board-text-muted mt-1">
        Order in epic: {(orderInEpic ?? 0) + 1}
      </div>
    </div>
  );
}
