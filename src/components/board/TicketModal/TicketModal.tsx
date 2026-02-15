import { useState, useEffect } from 'react';
import { getProjects } from '../../../lib/tauri';
import { logger } from '../../../lib/logger';
import { FullscreenDescriptionModal } from '../FullscreenDescriptionModal';
import { FullscreenCommentModal } from '../FullscreenCommentModal';
import { CreateCommentModal } from '../CreateCommentModal';
import { TaskList } from '../TaskList';
import type { Project, Comment } from '../../../types';
import type { TicketModalProps } from './types';
import { useTicketEdit } from './hooks/useTicketEdit';
import { useEpicData } from './hooks/useEpicData';
import { useRunsHistory } from './hooks/useRunsHistory';
import { useAgentEvents } from './hooks/useAgentEvents';
import { TicketModalHeader } from './TicketModalHeader';
import { TicketEditForm } from './TicketEditForm';
import { TicketDetails } from './TicketDetails';
import { DescriptionSection } from './DescriptionSection';
import { EpicPanel } from './EpicPanel';
import { AgentStatusPanel } from './AgentStatusPanel';
import { PausedTicketBanner } from './PausedTicketBanner';
import { BlockedTicketBanner } from './BlockedTicketBanner';
import { RunsHistory } from './RunsHistory';
import { TicketCostSummary } from './TicketCostSummary';
import { CommentsSection } from './CommentsSection';
import { TicketModalFooter } from './TicketModalFooter';
import { NextStepsPanel } from './NextStepsPanel';
import { useBoardStore } from '../../../stores/boardStore';

export function TicketModal({
  ticket,
  columns,
  comments,
  onClose,
  onUpdate,
  onAddComment,
  onUpdateComment,
  onRunWithAgent,
  onValidate,
  onDelete,
  onAgentComplete,
}: TicketModalProps) {
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectsLoading, setProjectsLoading] = useState(true);
  const [isFullscreenOpen, setIsFullscreenOpen] = useState(false);
  const [fullscreenComment, setFullscreenComment] = useState<Comment | null>(null);
  const [isCreateCommentModalOpen, setIsCreateCommentModalOpen] = useState(false);
  const [createCommentInitialContent, setCreateCommentInitialContent] = useState('');
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [commentClearTrigger, setCommentClearTrigger] = useState(0);

  const currentColumn = columns.find((c) => c.id === ticket.columnId);
  const tasks = useBoardStore((s) => s.tasks).filter((t) => t.ticketId === ticket.id);

  const editState = useTicketEdit({ ticket, onUpdate });
  
  const epicData = useEpicData({ ticket });
  
  const runsHistory = useRunsHistory({ 
    ticketId: ticket.id, 
    lockedByRunId: ticket.lockedByRunId 
  });
  
  const agentEvents = useAgentEvents({
    ticket,
    onAgentComplete,
    onUpdate,
    setAgentRuns: runsHistory.setAgentRuns,
    setEditBranchName: editState.setEditBranchName,
  });

  useEffect(() => {
    const loadProjects = async () => {
      try {
        setProjectsLoading(true);
        const data = await getProjects();
        setProjects(data);
      } catch (e) {
        logger.error('Failed to load projects:', e);
      } finally {
        setProjectsLoading(false);
      }
    };
    loadProjects();
  }, []);

  useEffect(() => {
    setShowDeleteConfirm(false);
  }, [ticket.id]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      if (showDeleteConfirm) {
        setShowDeleteConfirm(false);
      } else if (editState.isEditing) {
        editState.setIsEditing(false);
        editState.resetEditState();
      } else {
        onClose();
      }
    }
  };

  const handleDelete = async () => {
    if (!onDelete) return;
    try {
      await onDelete(ticket.id);
      onClose();
    } catch {
      // Don't close modal on error - let user see the failure and retry
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      onKeyDown={handleKeyDown}
    >
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative w-full max-w-[min(1200px,96vw)] max-h-[90vh] bg-board-column rounded-xl shadow-2xl overflow-hidden flex flex-col border border-board-border">
        {/* Header */}
        <TicketModalHeader
          ticket={ticket}
          currentColumn={currentColumn}
          isEditing={editState.isEditing}
          editTitle={editState.editTitle}
          setEditTitle={editState.setEditTitle}
          onClose={onClose}
        />

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          {/* Edit form fields (only visible when editing) */}
          {editState.isEditing && (
            <TicketEditForm
              projects={projects}
              projectsLoading={projectsLoading}
              editPriority={editState.editPriority}
              setEditPriority={editState.setEditPriority}
              editLabels={editState.editLabels}
              setEditLabels={editState.setEditLabels}
              editProjectId={editState.editProjectId}
              setEditProjectId={editState.setEditProjectId}
              editBranchName={editState.editBranchName}
              setEditBranchName={editState.setEditBranchName}
            />
          )}

          {/* Read-only details (only visible when not editing) */}
          {!editState.isEditing && (
            <TicketDetails ticket={ticket} projects={projects} />
          )}

          {/* Description */}
          <DescriptionSection
            description={ticket.descriptionMd}
            isEditing={editState.isEditing}
            editDescription={editState.editDescription}
            setEditDescription={editState.setEditDescription}
            onOpenFullscreen={() => setIsFullscreenOpen(true)}
          />

          {/* Blocked ticket banner (clarification needed) */}
          <BlockedTicketBanner
            ticket={ticket}
            columns={columns}
            comments={comments}
            tasks={tasks}
            onUpdate={onUpdate}
          />

          {/* Epic Panel */}
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

          {/* Task Queue - hide for epics since children ARE the tasks */}
          {!ticket.isEpic && <TaskList ticketId={ticket.id} />}

          {/* Paused ticket banner */}
          <PausedTicketBanner
            ticket={ticket}
            isTicketPaused={agentEvents.isTicketPaused}
            isResuming={agentEvents.isResuming}
            handleResumeTicket={() => agentEvents.handleResumeTicket(onClose)}
          />

          {/* Agent Status Panel */}
          <AgentStatusPanel
            lockedByRunId={ticket.lockedByRunId}
            agentError={agentEvents.agentError}
            setAgentError={agentEvents.setAgentError}
            isCancelling={agentEvents.isCancelling}
            isPausing={agentEvents.isPausing}
            handleCancelAgent={agentEvents.handleCancelAgent}
            handlePauseTicket={() => agentEvents.handlePauseTicket(runsHistory.agentRuns)}
            handleForceClearLock={agentEvents.handleForceClearLock}
          />

          {/* Next Steps Panel (visible for completed tickets with branches) */}
          <NextStepsPanel
            ticket={ticket}
            columns={columns}
            onValidate={onValidate}
          />

          {/* Ticket Cost Summary */}
          <TicketCostSummary ticketId={ticket.id} agentRuns={runsHistory.agentRuns} />

          {/* Runs History */}
          <RunsHistory
            agentRuns={runsHistory.agentRuns}
            lockedByRunId={ticket.lockedByRunId}
            expandedRunId={runsHistory.expandedRunId}
            runEvents={runsHistory.runEvents}
            loadingEvents={runsHistory.loadingEvents}
            handleRunClick={runsHistory.handleRunClick}
          />

          {/* Comments */}
          <CommentsSection
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
        </div>

        {/* Footer */}
        <TicketModalFooter
          ticket={ticket}
          currentColumn={currentColumn}
          isEditing={editState.isEditing}
          isSaving={editState.isSaving}
          showDeleteConfirm={showDeleteConfirm}
          setShowDeleteConfirm={setShowDeleteConfirm}
          onRunWithAgent={onRunWithAgent}
          onDelete={onDelete ? handleDelete : undefined}
          onSave={editState.handleSave}
          onCancelEdit={() => {
            editState.setIsEditing(false);
            editState.resetEditState();
          }}
          onStartEdit={() => editState.setIsEditing(true)}
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
            setFullscreenComment((prev) => prev ? { ...prev, bodyMd: newBody } : null);
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
          // Clear the inline comment input after successful modal submission
          setCommentClearTrigger((prev) => prev + 1);
        }}
        initialContent={createCommentInitialContent}
      />
    </div>
  );
}
