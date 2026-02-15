import { useState, useEffect, useCallback } from 'react';
import type { Ticket } from '../../../../types';

export interface UseTicketEditOptions {
  ticket: Ticket;
  onUpdate: (ticketId: string, updates: Partial<Ticket>) => Promise<void>;
}

export interface UseTicketEditReturn {
  isEditing: boolean;
  setIsEditing: (editing: boolean) => void;
  editTitle: string;
  setEditTitle: (title: string) => void;
  editDescription: string;
  setEditDescription: (desc: string) => void;
  editPriority: 'low' | 'medium' | 'high' | 'urgent';
  setEditPriority: (priority: 'low' | 'medium' | 'high' | 'urgent') => void;
  editLabels: string;
  setEditLabels: (labels: string) => void;
  editProjectId: string;
  setEditProjectId: (id: string) => void;
  editBranchName: string;
  setEditBranchName: (branch: string) => void;
  isSaving: boolean;
  handleSave: () => Promise<void>;
  resetEditState: () => void;
}

export function useTicketEdit({ ticket, onUpdate }: UseTicketEditOptions): UseTicketEditReturn {
  const [isEditing, setIsEditing] = useState(false);
  const [editTitle, setEditTitle] = useState(ticket.title);
  const [editDescription, setEditDescription] = useState(ticket.descriptionMd);
  const [editPriority, setEditPriority] = useState<'low' | 'medium' | 'high' | 'urgent'>(ticket.priority);
  const [editLabels, setEditLabels] = useState(ticket.labels.join(', '));
  const [editProjectId, setEditProjectId] = useState(ticket.projectId || '');
  const [editBranchName, setEditBranchName] = useState<string>(ticket.branchName || '');
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    setEditTitle(ticket.title);
    setEditDescription(ticket.descriptionMd);
    setEditPriority(ticket.priority);
    setEditLabels(ticket.labels.join(', '));
    setEditProjectId(ticket.projectId || '');
    setEditBranchName(ticket.branchName || '');
    setIsEditing(false);
  }, [ticket.id, ticket.pausedAt]);

  const resetEditState = useCallback(() => {
    setEditTitle(ticket.title);
    setEditDescription(ticket.descriptionMd);
    setEditPriority(ticket.priority);
    setEditLabels(ticket.labels.join(', '));
    setEditProjectId(ticket.projectId || '');
    setEditBranchName(ticket.branchName || '');
  }, [ticket]);

  const handleSave = useCallback(async () => {
    setIsSaving(true);
    try {
      const labels = editLabels
        .split(',')
        .map((l) => l.trim())
        .filter(Boolean);
      
      // NOTE: We intentionally omit columnId here. Saving edits should only
      // update content fields and never move the ticket between columns.
      // Column moves must be explicit (drag-and-drop, "Resolve & Move to Ready"
      // button, etc.) to avoid race conditions with the orchestrator which may
      // have moved the ticket (e.g. to Blocked for clarification) while the
      // user was editing.
      await onUpdate(ticket.id, {
        title: editTitle,
        descriptionMd: editDescription,
        priority: editPriority,
        labels,
        projectId: editProjectId,
        workflowType: 'multi_stage',
        branchName: editBranchName || undefined,
      });
      
      setIsEditing(false);
    } finally {
      setIsSaving(false);
    }
  }, [
    ticket.id,
    editTitle,
    editDescription,
    editPriority,
    editLabels,
    editProjectId,
    editBranchName,
    onUpdate,
  ]);

  return {
    isEditing,
    setIsEditing,
    editTitle,
    setEditTitle,
    editDescription,
    setEditDescription,
    editPriority,
    setEditPriority,
    editLabels,
    setEditLabels,
    editProjectId,
    setEditProjectId,
    editBranchName,
    setEditBranchName,
    isSaving,
    handleSave,
    resetEditState,
  };
}
