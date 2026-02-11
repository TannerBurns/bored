import { useState, useEffect, useCallback } from 'react';
import { useValidationStore } from '../../stores/validationStore';
import { ValidationChatView } from './ValidationChatView';
import type { ValidationSession, Ticket } from '../../types';
import { invoke } from '@tauri-apps/api/core';

interface ValidationViewProps {
  /** If provided, auto-opens validation for this ticket */
  initialTicketId?: string;
}

export function ValidationView({ initialTicketId }: ValidationViewProps) {
  const {
    currentSession,
    createSession,
    selectSession,
    deleteSession,
    isLoading,
    error,
  } = useValidationStore();

  const [tickets, setTickets] = useState<Record<string, Ticket>>({});
  const [showNewSession, setShowNewSession] = useState(false);
  const [newSessionTicketId, setNewSessionTicketId] = useState(initialTicketId || '');
  const [newSessionCommand, setNewSessionCommand] = useState('');
  const [newSessionPort, setNewSessionPort] = useState('');
  const [allSessions, setAllSessions] = useState<ValidationSession[]>([]);

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

  // Auto-create session for initial ticket
  useEffect(() => {
    if (initialTicketId && allSessions.length === 0) {
      // Check if there's already a session for this ticket
      const existing = allSessions.find(
        (s) => s.ticketId === initialTicketId && s.status !== 'passed' && s.status !== 'failed'
      );
      if (existing) {
        selectSession(existing);
      }
    }
  }, [initialTicketId, allSessions, selectSession]);

  const handleCreateSession = async () => {
    if (!newSessionTicketId) return;
    try {
      const session = await createSession({
        ticketId: newSessionTicketId,
        appCommand: newSessionCommand || undefined,
        appPort: newSessionPort ? parseInt(newSessionPort, 10) : undefined,
      });
      setShowNewSession(false);
      setNewSessionTicketId('');
      setNewSessionCommand('');
      setNewSessionPort('');
      selectSession(session);
      loadAllSessions();
    } catch {
      // Error handled in store
    }
  };

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
              Validate completed tickets with an AI agent
            </p>
          </div>
          <button
            onClick={() => setShowNewSession(true)}
            className="px-3 py-1.5 text-xs font-medium rounded-lg bg-board-accent hover:bg-board-accent-hover text-white transition-colors"
          >
            New Session
          </button>
        </div>

        {/* Error */}
        {error && (
          <div className="mx-6 mt-4 p-3 rounded-lg bg-red-500/10 text-red-400 text-xs">
            {error}
          </div>
        )}

        {/* New session form */}
        {showNewSession && (
          <div className="mx-6 mt-4 p-4 rounded-lg border border-board-border bg-board-hover/50 space-y-3">
            <h3 className="text-sm font-medium text-board-text">Create Validation Session</h3>
            <div className="space-y-2">
              <div>
                <label className="text-xs text-board-text-muted block mb-1">Ticket ID</label>
                <input
                  type="text"
                  value={newSessionTicketId}
                  onChange={(e) => setNewSessionTicketId(e.target.value)}
                  placeholder="ticket-id..."
                  className="w-full px-3 py-2 text-xs rounded-lg glass text-board-text placeholder:text-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent/50"
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="text-xs text-board-text-muted block mb-1">App Command (optional)</label>
                  <input
                    type="text"
                    value={newSessionCommand}
                    onChange={(e) => setNewSessionCommand(e.target.value)}
                    placeholder="npm run dev"
                    className="w-full px-3 py-2 text-xs rounded-lg glass text-board-text placeholder:text-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent/50"
                  />
                </div>
                <div>
                  <label className="text-xs text-board-text-muted block mb-1">Port (optional)</label>
                  <input
                    type="number"
                    value={newSessionPort}
                    onChange={(e) => setNewSessionPort(e.target.value)}
                    placeholder="3000"
                    className="w-full px-3 py-2 text-xs rounded-lg glass text-board-text placeholder:text-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent/50"
                  />
                </div>
              </div>
            </div>
            <div className="flex gap-2 justify-end">
              <button
                onClick={() => setShowNewSession(false)}
                className="px-3 py-1.5 text-xs rounded-lg bg-board-hover text-board-text-muted hover:text-board-text transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleCreateSession}
                disabled={!newSessionTicketId || isLoading}
                className="px-3 py-1.5 text-xs font-medium rounded-lg bg-board-accent hover:bg-board-accent-hover text-white transition-colors disabled:opacity-50"
              >
                {isLoading ? 'Creating...' : 'Create'}
              </button>
            </div>
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
                Create a session from a completed ticket's "Next Steps" panel, or click "New Session" above.
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
                        {session.appCommand && <span>{session.appCommand} </span>}
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
