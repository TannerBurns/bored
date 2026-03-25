import { invoke } from '@tauri-apps/api/core';
import { useBoardStore } from '../stores/boardStore';
import { useSettingsStore, ensureAgentConfigsSynced } from '../stores/settingsStore';
import { deleteTicket, startAgentRun, getWorkspaceProjects } from '../lib/tauri';
import { logger } from '../lib/logger';
import type { Ticket, Project, CreateTicketInput } from '../types';

interface UseTicketHandlersParams {
  tickets: Ticket[];
  setTickets: React.Dispatch<React.SetStateAction<Ticket[]>>;
  projects: Project[];
}

export function useTicketHandlers({ tickets, setTickets, projects }: UseTicketHandlersParams) {
  const selectedTicket = useBoardStore((s) => s.selectedTicket);
  const openTicketModal = useBoardStore((s) => s.openTicketModal);
  const closeTicketModal = useBoardStore((s) => s.closeTicketModal);
  const addComment = useBoardStore((s) => s.addComment);
  const updateComment = useBoardStore((s) => s.updateComment);
  const storeCreateTicket = useBoardStore((s) => s.createTicket);
  const storeUpdateTicket = useBoardStore((s) => s.updateTicket);
  const storeMoveTicket = useBoardStore((s) => s.moveTicket);

  const handleTicketMove = async (ticketId: string, newColumnId: string) => {
    const updatedAt = new Date().toISOString();
    const originalTickets = tickets;
    setTickets((prev) =>
      prev.map((t) =>
        t.id === ticketId ? { ...t, columnId: newColumnId, updatedAt } : t
      )
    );
    try {
      await storeMoveTicket(ticketId, newColumnId, updatedAt);
    } catch (error) {
      logger.error('Failed to move ticket:', error);
      setTickets(originalTickets);
      throw error;
    }
  };

  const handleTicketClick = (ticket: Ticket) => openTicketModal(ticket);

  const handleCreateTicket = async (input: CreateTicketInput) => {
    const ticket = await storeCreateTicket(input);
    setTickets((prev) => [...prev, ticket]);
    return ticket;
  };

  const handleUpdateTicket = async (ticketId: string, updates: Partial<Ticket>) => {
    const updatedAt = new Date().toISOString();
    const updatesWithTimestamp = { ...updates, updatedAt };
    const originalTickets = tickets;
    setTickets((prev) =>
      prev.map((t) =>
        t.id === ticketId ? { ...t, ...updatesWithTimestamp } : t
      )
    );
    try {
      await storeUpdateTicket(ticketId, updatesWithTimestamp);
    } catch (error) {
      logger.error('Failed to update ticket:', error);
      setTickets(originalTickets);
    }
  };

  const handleAddComment = async (ticketId: string, body: string) => {
    await addComment(ticketId, body);
  };

  const handleUpdateComment = async (commentId: string, body: string) => {
    await updateComment(commentId, body);
  };

  const handleRunWithAgent = async (ticketId: string, agentType: string, workflowMode?: string) => {
    logger.debug('handleRunWithAgent called', { ticketId, agentType, workflowMode });
    
    const ticket = tickets.find(t => t.id === ticketId);
    if (!ticket) {
      logger.error('Ticket not found:', ticketId);
      return;
    }
    
    let projectPath: string;
    if (ticket.projectId) {
      const project = projects.find(p => p.id === ticket.projectId);
      if (!project) {
        logger.error('Project not found:', ticket.projectId);
        return;
      }
      projectPath = project.path;
    } else if (ticket.workspaceId) {
      const wsProjects = await getWorkspaceProjects(ticket.workspaceId);
      if (wsProjects.length === 0) {
        logger.error('Workspace has no projects:', ticket.workspaceId);
        return;
      }
      projectPath = wsProjects[0].path;
    } else {
      logger.error('Ticket has no projectId or workspaceId:', ticketId);
      return;
    }
    
    logger.debug('Starting agent with project path', { path: projectPath });
    
    await ensureAgentConfigsSynced();

    const { agentConfigs } = useSettingsStore.getState();
    const cfg = agentConfigs[agentType] ?? agentConfigs['claude'];
    
    try {
      logger.debug('Calling startAgentRun...');

      const runId = await startAgentRun(ticketId, agentType, projectPath, {
        codeReviewMaxIterations: cfg.codeReviewMaxIterations,
        stageTimeoutHours: cfg.stageTimeoutHours,
        stageMaxRetries: cfg.stageMaxRetries,
        stageConfigs: cfg.workflowStages,
        workflowMode,
      });
      logger.info('Agent run started', { runId });
      
      const updates = { lockedByRunId: runId, updatedAt: new Date().toISOString() };
      logger.debug('Updating ticket with lockedByRunId', { runId });
      
      setTickets((prev) =>
        prev.map((t) => (t.id === ticketId ? { ...t, ...updates } : t))
      );
      
      await storeUpdateTicket(ticketId, updates);
      logger.debug('Ticket updated, modal should now show agent running');
    } catch (err) {
      logger.error('Failed to start agent:', err);
    }
  };

  const handleDeleteTicket = async (ticketId: string) => {
    await deleteTicket(ticketId);
    setTickets((prev) => prev.filter((t) => t.id !== ticketId));
    closeTicketModal();
  };

  const handleAgentComplete = async (runId: string, status: string) => {
    logger.info('Agent run completed', { runId, status });
    if (selectedTicket) {
      const updatedAt = new Date().toISOString();
      const updates = { lockedByRunId: null as string | null, updatedAt };

      // Update local tickets (useBoardSync state)
      setTickets((prev) =>
        prev.map((t) => (t.id === selectedTicket.id ? { ...t, ...updates } : t))
      );

      // Update store tickets + selectedTicket in a SINGLE synchronous set
      // so useAgentEvents sees lockedByRunId=null immediately and doesn't
      // bounce isAgentRunning back to true.
      useBoardStore.setState((state) => ({
        tickets: state.tickets.map((t) =>
          t.id === selectedTicket.id ? { ...t, ...updates } : t
        ),
        selectedTicket:
          state.selectedTicket?.id === selectedTicket.id
            ? { ...state.selectedTicket, ...updates }
            : state.selectedTicket,
      }));

      // Persist to backend (fire-and-forget; store already up-to-date)
      try {
        await invoke('update_ticket', {
          ticketId: selectedTicket.id,
          updates: { lockedByRunId: null, updatedAt },
        });
      } catch (error) {
        logger.error('Failed to persist ticket update after agent complete:', error);
      }
    }
  };

  return {
    handleTicketMove,
    handleTicketClick,
    handleCreateTicket,
    handleUpdateTicket,
    handleAddComment,
    handleUpdateComment,
    handleRunWithAgent,
    handleDeleteTicket,
    handleAgentComplete,
  };
}
