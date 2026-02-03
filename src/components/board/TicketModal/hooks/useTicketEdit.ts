import { useState, useEffect, useCallback } from 'react';
import type { Ticket } from '../../../../types';

export interface UseTicketEditOptions {
  ticket: Ticket;
  onUpdate: (ticketId: string, updates: Partial<Ticket>) => Promise<void>;
}

export interface UseTicketEditReturn {
  // Edit state
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
  editAgentPref: 'cursor' | 'claude' | 'any';
  setEditAgentPref: (pref: 'cursor' | 'claude' | 'any') => void;
  editModel: string;
  setEditModel: (model: string) => void;
  editBranchName: string;
  setEditBranchName: (branch: string) => void;
  editColumnId: string;
  setEditColumnId: (columnId: string) => void;
  
  // Save state
  isSaving: boolean;
  
  // Handlers
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
  const [editAgentPref, setEditAgentPref] = useState<'cursor' | 'claude' | 'any'>(ticket.agentPref || 'any');
  const [editModel, setEditModel] = useState<string>(ticket.model || '');
  const [editBranchName, setEditBranchName] = useState<string>(ticket.branchName || '');
  const [editColumnId, setEditColumnId] = useState<string>(ticket.columnId);
  const [isSaving, setIsSaving] = useState(false);

  // Reset edit state when the ticket prop changes (e.g., user selects a different ticket)
  useEffect(() => {
    setEditTitle(ticket.title);
    setEditDescription(ticket.descriptionMd);
    setEditPriority(ticket.priority);
    setEditLabels(ticket.labels.join(', '));
    setEditProjectId(ticket.projectId || '');
    setEditAgentPref(ticket.agentPref || 'any');
    setEditModel(ticket.model || '');
    setEditBranchName(ticket.branchName || '');
    setEditColumnId(ticket.columnId);
    setIsEditing(false);
  }, [ticket.id, ticket.pausedAt]);

  const resetEditState = useCallback(() => {
    setEditTitle(ticket.title);
    setEditDescription(ticket.descriptionMd);
    setEditPriority(ticket.priority);
    setEditLabels(ticket.labels.join(', '));
    setEditProjectId(ticket.projectId || '');
    setEditAgentPref(ticket.agentPref || 'any');
    setEditModel(ticket.model || '');
    setEditBranchName(ticket.branchName || '');
    setEditColumnId(ticket.columnId);
  }, [ticket]);

  const handleSave = useCallback(async () => {
    setIsSaving(true);
    try {
      const labels = editLabels
        .split(',')
        .map((l) => l.trim())
        .filter(Boolean);
      
      await onUpdate(ticket.id, {
        title: editTitle,
        descriptionMd: editDescription,
        priority: editPriority,
        labels,
        projectId: editProjectId,
        workflowType: 'multi_stage',
        agentPref: editAgentPref,
        model: editModel || undefined,
        branchName: editBranchName || undefined,
        columnId: editColumnId,
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
    editAgentPref,
    editModel,
    editBranchName,
    editColumnId,
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
    editAgentPref,
    setEditAgentPref,
    editModel,
    setEditModel,
    editBranchName,
    setEditBranchName,
    editColumnId,
    setEditColumnId,
    isSaving,
    handleSave,
    resetEditState,
  };
}
