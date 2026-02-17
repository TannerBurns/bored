import { useState, useEffect, useCallback, useRef } from 'react';
import { useValidationStore } from '../../stores/validationStore';
import { ValidationChatView } from './ValidationChatView';
import type { ValidationSession, Ticket } from '../../types';
import { invoke } from '@tauri-apps/api/core';
import { getTicket } from '../../lib/tauri';

interface ValidationViewProps {
  /** When both set, auto-create a session and open chat (from ticket Next Steps) */
  initialTicketId?: string;
  initialAgentType?: string;
  /** Called after auto-created session is opened so parent can clear initial state */
  onConsumedInitial?: () => void;
}

export function ValidationView({
  initialTicketId,
  initialAgentType,
  onConsumedInitial,
}: ValidationViewProps) {
  const {
    currentSession,
    createSession,
    selectSession,
    deleteSession,
    isLoading,
    error,
  } = useValidationStore();

  const [tickets, setTickets] = useState<Record<string, Ticket>>({});
  const [allSessions, setAllSessions] = useState<ValidationSession[]>([]);
  const initialConsumedRef = useRef(false);

  // Load all sessions on mount
  const loadAllSessions = useCallback(async () => {
    try {
      // Get all boards to find all tickets
      const boards: { id: string }[] = await invoke('get_boards');
      const sessionPromises: ValidationSession[][] = [];
      const ticketMap: Record<string, Ticket> = {};

      for (const board of boards) {
        const boardTickets: Ticket[] = await invoke('get_tickets', { boardId: board.id });
        for (const ticket of boardTickets) {
          ticketMap[ticket.id] = ticket;
          if (ticket.branchName) {
            const ticketSessions = await invoke('get_validation_sessions', {
              ticketId: ticket.id,
            }) as ValidationSession[];
            if (ticketSessions.length > 0) {
              sessionPromises.push(ticketSessions);
            }
          }
        }
      }

      setTickets(ticketMap);
      const flatSessions = sessionPromises.flat();
      flatSessions.sort(
        (a, b) =>
          new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
      );
      setAllSessions(flatSessions);
    } catch {
      // Ignore errors loading all sessions
    }
  }, []);

  useEffect(() => {
    loadAllSessions();
  }, [loadAllSessions]);

  // Auto-create session when opened from ticket Next Steps (initialTicketId + initialAgentType)
  useEffect(() => {
    if (
      !initialTicketId ||
      !initialAgentType ||
      initialConsumedRef.current ||
      isLoading
    ) {
      return;
    }
    initialConsumedRef.current = true;

    const run = async () => {
      try {
        const ticket = await getTicket(initialTicketId);
        const session = await createSession({
          ticketId: initialTicketId,
          projectId: ticket.projectId ?? undefined,
          agentType: initialAgentType,
        });
        selectSession(session);
        loadAllSessions();
        onConsumedInitial?.();
      } catch {
        initialConsumedRef.current = false;
      }
    };
    void run();
  }, [
    initialTicketId,
    initialAgentType,
    isLoading,
    createSession,
    selectSession,
    loadAllSessions,
    onConsumedInitial,
  ]);

  const handleDeleteSession = async (sessionId: string) => {
    await deleteSession(sessionId);
    loadAllSessions();
  };

  const statusColors: Record<string, string> = {
    created: 'bg-gray-500/20 text-gray-400',
    chatting: 'bg-blue-500/20 text-blue-400',
    app_running: 'bg-emerald-500/20 text-emerald-400',
    passed: 'bg-emerald-500/20 text-emerald-400',
    failed: 'bg-red-500/20 text-red-400',
  };

  const statusLabels: Record<string, string> = {
    created: 'Ready',
    chatting: 'Chatting',
    app_running: 'Running',
    passed: 'Passed',
    failed: 'Needs Fix',
  };

  // If we have a current session, show the chat view
  if (currentSession) {
    return (
      <div className="flex-1 overflow-hidden flex gap-4">
        <div className="flex-1 glass rounded-xl overflow-hidden">
          <ValidationChatView
            session={currentSession}
            onBack={() => selectSession(null)}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-hidden flex gap-4">
      {/* Main content */}
      <div className="flex-1 glass rounded-xl overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-board-border">
          <div>
            <h2 className="text-lg font-semibold text-board-text">Validation</h2>
            <p className="text-xs text-board-text-muted mt-0.5">
              Validate completed tickets from a ticket&apos;s &quot;Work Complete&quot; panel
            </p>
          </div>
        </div>

        {/* Error */}
        {error && (
          <div className="mx-6 mt-4 p-3 rounded-lg bg-red-500/10 text-red-400 text-xs">
            {error}
          </div>
        )}

        {/* Sessions list */}
        <div className="flex-1 overflow-y-auto p-6">
          {allSessions.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-64 text-board-text-muted">
              <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="mb-4 opacity-40">
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
                <polyline points="22 4 12 14.01 9 11.01" />
              </svg>
              <p className="text-sm">No validation sessions yet</p>
              <p className="text-xs mt-1">
                Move a ticket to Done or Review and use &quot;Build with&quot; to validate from the Work Complete panel.
              </p>
            </div>
          ) : (
            <div className="space-y-2">
              {allSessions.map((session) => {
                const ticket = tickets[session.ticketId];
                return (
                  <div
                    key={session.id}
                    className="flex items-center gap-3 p-3 rounded-lg border border-board-border hover:bg-board-hover/50 cursor-pointer transition-colors group"
                    onClick={() => selectSession(session)}
                  >
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-board-text truncate">
                          {ticket?.title || session.ticketId}
                        </span>
                        <span
                          className={`px-1.5 py-0.5 text-xs rounded-full ${
                            statusColors[session.status] || statusColors.created
                          }`}
                        >
                          {statusLabels[session.status] || session.status}
                        </span>
                      </div>
                      <div className="text-xs text-board-text-muted mt-0.5">
                        <span>Created {new Date(session.createdAt).toLocaleDateString()}</span>
                      </div>
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDeleteSession(session.id);
                      }}
                      className="p-1 opacity-0 group-hover:opacity-100 text-board-text-muted hover:text-red-400 transition-all"
                    >
                      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <polyline points="3 6 5 6 21 6" />
                        <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                      </svg>
                    </button>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
