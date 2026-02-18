import { useBoardStore } from '../stores/boardStore';
import { useSettingsStore, ensureAgentConfigsSynced } from '../stores/settingsStore';
import { deleteTicket, startAgentRun } from '../lib/tauri';
import { logger } from '../lib/logger';
import type { Ticket, Project, CreateTicketInput } from '../types';

interface UseTicketHandlersParams {
  tickets: Ticket[];
  setTickets: React.Dispatch<React.SetStateAction<Ticket[]>>;
  projects: Project[];
}

export function useTicketHandlers({ tickets, setTickets, projects }: UseTicketHandlersParams) {
  const {
    selectedTicket,
    openTicketModal,
    closeTicketModal,
    addComment,
    updateComment,
    createTicket: storeCreateTicket,
    updateTicket: storeUpdateTicket,
    moveTicket: storeMoveTicket,
  } = useBoardStore();

  const handleTicketMove = async (ticketId: string, newColumnId: string) => {
    const updatedAt = new Date();
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
    }
  };

  const handleTicketClick = (ticket: Ticket) => openTicketModal(ticket);

  const handleCreateTicket = async (input: CreateTicketInput) => {
    const ticket = await storeCreateTicket(input);
    setTickets((prev) => [...prev, ticket]);
    return ticket;
  };

  const handleUpdateTicket = async (ticketId: string, updates: Partial<Ticket>) => {
    const updatedAt = new Date();
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

  const handleRunWithAgent = async (ticketId: string, agentType: string) => {
    logger.debug('handleRunWithAgent called', { ticketId, agentType });
    
    const ticket = tickets.find(t => t.id === ticketId);
    if (!ticket) {
      logger.error('Ticket not found:', ticketId);
      return;
    }
    
    if (!ticket.projectId) {
      logger.error('Ticket has no projectId:', ticketId);
      return;
    }
    
    const project = projects.find(p => p.id === ticket.projectId);
    if (!project) {
      logger.error('Project not found:', ticket.projectId);
      return;
    }
    
    logger.debug('Starting agent with project', { projectId: project.id, path: project.path });
    
    await ensureAgentConfigsSynced();

    const { agentConfigs } = useSettingsStore.getState();
    const cfg = agentConfigs[agentType] ?? agentConfigs['claude'];
    
    try {
      logger.debug('Calling startAgentRun...');
      const runId = await startAgentRun(ticketId, agentType, project.path, {
        codeReviewMaxIterations: cfg.codeReviewMaxIterations,
        stageTimeoutHours: cfg.stageTimeoutHours,
        stageMaxRetries: cfg.stageMaxRetries,
        stageConfigs: cfg.workflowStages,
      });
      logger.info('Agent run started', { runId });
      
      const updates = { lockedByRunId: runId, updatedAt: new Date() };
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
      const updates = { lockedByRunId: undefined, updatedAt: new Date() };
      setTickets((prev) =>
        prev.map((t) => (t.id === selectedTicket.id ? { ...t, ...updates } : t))
      );
      await storeUpdateTicket(selectedTicket.id, updates);
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
