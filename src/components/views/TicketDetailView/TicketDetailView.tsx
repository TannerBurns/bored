import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { getProjects, getWorkspaces } from '../../../lib/tauri';
import { useChatStore } from '../../../stores/chatStore';
import { logger } from '../../../lib/logger';
import { cn } from '../../../lib/utils';
import { FullscreenDescriptionModal } from '../../board/FullscreenDescriptionModal';
import { FullscreenCommentModal } from '../../board/FullscreenCommentModal';
import { CreateCommentModal } from '../../board/CreateCommentModal';
import { useBoardStore } from '../../../stores/boardStore';
import { validateTransition } from '../../board/TransitionGuard';
import { useTicketEdit } from '../../board/TicketModal/hooks/useTicketEdit';
import { useEpicData } from '../../board/TicketModal/hooks/useEpicData';
import { useRunsHistory } from '../../board/TicketModal/hooks/useRunsHistory';
import { useAgentEvents } from '../../board/TicketModal/hooks/useAgentEvents';
import { TicketDetailHeader } from './TicketDetailHeader';
import { TicketDetailSidebar } from './TicketDetailSidebar';
import { OverviewTab } from './OverviewTab';
import { TasksTab } from './TasksTab';
import { AgentTab } from './AgentTab';
import { ActivityTab } from './ActivityTab';
import type { Ticket, Column, Comment, Project, Workspace } from '../../../types';

type TabId = 'overview' | 'tasks' | 'agent' | 'activity';

const TABS: { id: TabId; label: string }[] = [
  { id: 'overview', label: 'Overview' },
  { id: 'tasks', label: 'Task' },
  { id: 'agent', label: 'Agent' },
  { id: 'activity', label: 'Activity' },
];

export interface TicketDetailViewProps {
  ticket: Ticket;
  columns: Column[];
  comments: Comment[];
  boardName: string;
  onClose: () => void;
  onUpdate: (ticketId: string, updates: Partial<Ticket>) => Promise<void>;
  onMoveTicket: (ticketId: string, newColumnId: string) => void | Promise<void>;
  onAddComment: (ticketId: string, body: string) => Promise<void>;
  onUpdateComment: (commentId: string, body: string) => Promise<void>;
  onRunWithAgent?: (ticketId: string, agentType: string, workflowMode?: string) => void;
  onNavigateToChat?: () => void;
  onDelete?: (ticketId: string) => Promise<void>;
  onAgentComplete?: (runId: string, status: string) => void;
}

export function TicketDetailView({
  ticket,
  columns,
  comments,
  boardName,
  onClose,
  onUpdate,
  onMoveTicket,
  onAddComment,
  onUpdateComment,
  onRunWithAgent,
  onNavigateToChat,
  onDelete,
  onAgentComplete,
}: TicketDetailViewProps) {
  const [activeTab, setActiveTab] = useState<TabId>('overview');
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [isFullscreenOpen, setIsFullscreenOpen] = useState(false);
  const [fullscreenComment, setFullscreenComment] = useState<Comment | null>(null);
  const [isCreateCommentModalOpen, setIsCreateCommentModalOpen] = useState(false);
  const [createCommentInitialContent, setCreateCommentInitialContent] = useState('');
  const [commentClearTrigger, setCommentClearTrigger] = useState(0);

  const tickets = useBoardStore((s) => s.tickets);
  const allTasks = useBoardStore((s) => s.tasks);
  const openTicketModal = useBoardStore((s) => s.openTicketModal);

  const tasks = useMemo(
    () => allTasks.filter((t) => t.ticketId === ticket.id),
    [allTasks, ticket.id]
  );

  const editState = useTicketEdit({ ticket, onUpdate });
  const epicData = useEpicData({ ticket });
  const runsHistory = useRunsHistory({
    ticketId: ticket.id,
    lockedByRunId: ticket.lockedByRunId,
  });
  const agentEvents = useAgentEvents({
    ticket,
    onAgentComplete,
    onUpdate,
    setAgentRuns: runsHistory.setAgentRuns,
    setEditBranchName: editState.setEditBranchName,
  });

  const createChat = useChatStore((s) => s.createChat);
  const selectChat = useChatStore((s) => s.selectChat);

  const handleValidateWithAgent = useCallback(async (agentType: string) => {
    try {
      const chat = await createChat({
        agentType,
        projectId: ticket.projectId,
        workspaceId: ticket.workspaceId,
        mode: 'review' as const,
        boardId: ticket.boardId,
        ticketId: ticket.id,
      });
      await selectChat(chat.id);
      if (onNavigateToChat) {
        onNavigateToChat();
      } else {
        onClose();
      }
    } catch (e) {
      logger.error('Failed to create validation chat:', e);
    }
  }, [ticket, createChat, selectChat, onNavigateToChat, onClose]);

  // Auto-switch to Agent tab when a run starts
  useEffect(() => {
    if (ticket.lockedByRunId) {
      setActiveTab('agent');
    }
  }, [ticket.lockedByRunId]);

  // Show Activity tab badge only when ticket is blocked and needs user input
  const currentColumn = columns.find((c) => c.id === ticket.columnId);
  const isBlocked = currentColumn?.name.toLowerCase() === 'blocked';
  const needsClarification = useMemo(() => {
    if (!isBlocked) return false;
    const nonUserComments = comments
      .filter((c) => c.ticketId === ticket.id && c.authorType !== 'user')
      .sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime());
    const latestDiagnostic = nonUserComments.find((c) => c.metadata?.type === 'diagnostic');
    const latestClarification = nonUserComments.find((c) => c.metadata?.type === 'clarification');
    if (!latestClarification) return false;
    if (latestDiagnostic && new Date(latestDiagnostic.createdAt).getTime() > new Date(latestClarification.createdAt).getTime()) return false;
    return true;
  }, [comments, ticket.id, isBlocked]);

  // Pending task count for Tasks tab badge
  const pendingTaskCount = useMemo(() => {
    return tasks.filter((t) => t.status === 'pending').length;
  }, [tasks]);

  useEffect(() => {
    const load = async () => {
      try {
        const [projectsData, workspacesData] = await Promise.all([
          getProjects(),
          getWorkspaces(),
        ]);
        setProjects(projectsData);
        setWorkspaces(workspacesData);
      } catch (e) {
        logger.error('Failed to load projects/workspaces:', e);
      }
    };
    load();
  }, []);

  // Prev/Next ticket navigation within the same column
  const columnTickets = useMemo(() => {
    return tickets
      .filter((t) => t.columnId === ticket.columnId)
      .sort((a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime());
  }, [tickets, ticket.columnId]);

  const currentIndex = columnTickets.findIndex((t) => t.id === ticket.id);

  const handlePrev = useCallback(() => {
    if (currentIndex > 0) {
      openTicketModal(columnTickets[currentIndex - 1]);
    }
  }, [currentIndex, columnTickets, openTicketModal]);

  const handleNext = useCallback(() => {
    if (currentIndex < columnTickets.length - 1) {
      openTicketModal(columnTickets[currentIndex + 1]);
    }
  }, [currentIndex, columnTickets, openTicketModal]);

  const { isEditing, setIsEditing, resetEditState } = editState;

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement ||
        e.target instanceof HTMLSelectElement
      ) {
        return;
      }

      if (e.key === 'Escape') {
        if (isEditing) {
          setIsEditing(false);
          resetEditState();
        } else {
          onClose();
        }
      } else if (e.key === 'ArrowLeft' && e.altKey) {
        if (currentIndex > 0) handlePrev();
      } else if (e.key === 'ArrowRight' && e.altKey) {
        if (currentIndex < columnTickets.length - 1) handleNext();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [
    isEditing,
    setIsEditing,
    resetEditState,
    onClose,
    currentIndex,
    columnTickets.length,
    handlePrev,
    handleNext,
  ]);

  return (
    <div className="flex flex-col h-full min-h-0">
      {/* Header with breadcrumbs */}
      <TicketDetailHeader
        ticket={ticket}
        boardName={boardName}
        isEditing={editState.isEditing}
        editTitle={editState.editTitle}
        setEditTitle={editState.setEditTitle}
        onBack={onClose}
        onPrev={currentIndex > 0 ? handlePrev : null}
        onNext={currentIndex < columnTickets.length - 1 ? handleNext : null}
      />

      {/* Main content area: tabs + sidebar */}
      <div className="flex flex-1 min-h-0 gap-0">
        {/* Left: Tab content */}
        <div className="flex-1 flex flex-col min-h-0 min-w-0">
          {/* Tab bar */}
          <div
            className="flex-shrink-0 flex items-center gap-1 border-b border-board-border px-1 mb-0"
            role="tablist"
            onKeyDown={(e) => {
              const idx = TABS.findIndex((t) => t.id === activeTab);
              let next: number | null = null;
              if (e.key === 'ArrowRight') next = (idx + 1) % TABS.length;
              else if (e.key === 'ArrowLeft') next = (idx - 1 + TABS.length) % TABS.length;
              else if (e.key === 'Home') next = 0;
              else if (e.key === 'End') next = TABS.length - 1;
              if (next !== null) {
                e.preventDefault();
                setActiveTab(TABS[next].id);
                tabRefs.current[next]?.focus();
              }
            }}
          >
            {TABS.map((tab, i) => (
              <button
                key={tab.id}
                ref={(el) => { tabRefs.current[i] = el; }}
                role="tab"
                id={`tab-${tab.id}`}
                aria-selected={activeTab === tab.id}
                aria-controls={`tabpanel-${tab.id}`}
                tabIndex={activeTab === tab.id ? 0 : -1}
                onClick={() => setActiveTab(tab.id)}
                className={cn(
                  'relative px-4 py-2.5 text-sm font-medium transition-colors',
                  activeTab === tab.id
                    ? 'text-board-accent'
                    : 'text-board-text-muted hover:text-board-text'
                )}
              >
                <span className="flex items-center gap-1.5">
                  {tab.label}
                  {tab.id === 'tasks' && pendingTaskCount > 0 && (
                    <span className="min-w-[18px] h-[18px] flex items-center justify-center rounded-full bg-board-accent/20 text-board-accent text-[10px] font-bold px-1">
                      {pendingTaskCount}
                    </span>
                  )}
                  {tab.id === 'agent' && ticket.lockedByRunId && (
                    <span className="w-2 h-2 rounded-full bg-status-warning animate-pulse" />
                  )}
                  {tab.id === 'activity' && needsClarification && (
                    <span className="w-2 h-2 rounded-full bg-status-error" />
                  )}
                </span>
                {activeTab === tab.id && (
                  <span className="absolute bottom-0 left-2 right-2 h-0.5 bg-board-accent rounded-full" />
                )}
              </button>
            ))}
          </div>

          {/* Tab content (scrollable) */}
          <div
            className="flex-1 overflow-y-auto p-4"
            role="tabpanel"
            id={`tabpanel-${activeTab}`}
            aria-labelledby={`tab-${activeTab}`}
            tabIndex={0}
          >
            {activeTab === 'overview' && (
              <OverviewTab
                ticket={ticket}
                columns={columns}
                comments={comments}
                tasks={tasks}
                editState={editState}
                agentEvents={agentEvents}
                onUpdate={onUpdate}
                onOpenFullscreen={() => setIsFullscreenOpen(true)}
                onBack={onClose}
              />
            )}

            {activeTab === 'tasks' && (
              <TasksTab
                ticket={ticket}
                columns={columns}
                epicData={epicData}
              />
            )}

            {activeTab === 'agent' && (
              <AgentTab
                ticket={ticket}
                agentEvents={agentEvents}
                runsHistory={runsHistory}
              />
            )}

            {activeTab === 'activity' && (
              <ActivityTab
                ticketId={ticket.id}
                comments={comments}
                onAddComment={onAddComment}
                onOpenFullscreenComment={setFullscreenComment}
                onOpenCreateCommentModal={(initialContent) => {
                  setCreateCommentInitialContent(initialContent);
                  setIsCreateCommentModalOpen(true);
                }}
                clearInputTrigger={commentClearTrigger}
              />
            )}
          </div>
        </div>

        {/* Right sidebar */}
        <TicketDetailSidebar
          ticket={ticket}
          columns={columns}
          projects={projects}
          workspaces={workspaces}
          agentRuns={runsHistory.agentRuns}
          editState={editState}
          parentEpic={epicData.parentEpic}
          onMoveTicket={(newColumnId) => {
            const validation = validateTransition(ticket, columns, newColumnId, tasks.length);
            if (!validation.valid) {
              logger.error(validation.reason ?? 'Invalid transition');
              return;
            }
            onMoveTicket(ticket.id, newColumnId);
          }}
          onRunWithAgent={onRunWithAgent}
          onValidateWithAgent={handleValidateWithAgent}
          onDelete={onDelete}
          onBack={onClose}
        />
      </div>

      {/* Fullscreen Description Modal */}
      <FullscreenDescriptionModal
        description={ticket.descriptionMd}
        isOpen={isFullscreenOpen}
        onClose={() => setIsFullscreenOpen(false)}
        onSave={async (newDescription) => {
          await onUpdate(ticket.id, { descriptionMd: newDescription });
          editState.setEditDescription(newDescription);
        }}
        ticketTitle={ticket.title}
      />

      {/* Fullscreen Comment Modal */}
      {fullscreenComment && (
        <FullscreenCommentModal
          comment={fullscreenComment}
          isOpen={!!fullscreenComment}
          onClose={() => setFullscreenComment(null)}
          onSave={async (commentId, newBody) => {
            await onUpdateComment(commentId, newBody);
            setFullscreenComment((prev) =>
              prev ? { ...prev, bodyMd: newBody } : null
            );
          }}
        />
      )}

      {/* Create Comment Modal */}
      <CreateCommentModal
        isOpen={isCreateCommentModalOpen}
        onClose={() => {
          setIsCreateCommentModalOpen(false);
          setCreateCommentInitialContent('');
        }}
        onSubmit={async (body) => {
          await onAddComment(ticket.id, body);
          setCommentClearTrigger((prev) => prev + 1);
        }}
        initialContent={createCommentInitialContent}
      />
    </div>
  );
}
