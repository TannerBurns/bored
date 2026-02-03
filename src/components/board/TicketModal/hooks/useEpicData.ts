import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { logger } from '../../../../lib/logger';
import type { Ticket as TicketType, EpicProgress } from '../../../../types';

export interface UseEpicDataOptions {
  ticket: TicketType;
}

export interface UseEpicDataReturn {
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

export function useEpicData({ ticket }: UseEpicDataOptions): UseEpicDataReturn {
  const [epicChildren, setEpicChildren] = useState<TicketType[]>([]);
  const [epicProgress, setEpicProgress] = useState<EpicProgress | null>(null);
  const [parentEpic, setParentEpic] = useState<TicketType | null>(null);
  const [loadingEpic, setLoadingEpic] = useState(false);
  const [availableTickets, setAvailableTickets] = useState<TicketType[]>([]);
  const [selectedChildId, setSelectedChildId] = useState<string>('');
  const [isAddingChild, setIsAddingChild] = useState(false);

  // Load epic-related data
  useEffect(() => {
    const loadEpicData = async () => {
      setLoadingEpic(true);
      try {
        if (ticket.isEpic) {
          // This is an epic - load children, progress, and available tickets
          const [children, progress, allTickets] = await Promise.all([
            invoke<TicketType[]>('get_epic_children', { epicId: ticket.id }),
            invoke<EpicProgress>('get_epic_progress', { epicId: ticket.id }),
            invoke<TicketType[]>('get_tickets', { boardId: ticket.boardId }),
          ]);
          setEpicChildren(children);
          setEpicProgress(progress);
          setParentEpic(null);
          
          // Filter available tickets: not an epic, not already a child, not this ticket
          const available = allTickets.filter(t => 
            !t.isEpic && 
            !t.epicId && 
            t.id !== ticket.id
          );
          setAvailableTickets(available);
        } else if (ticket.epicId) {
          // This is a child - load parent epic
          try {
            const tickets = await invoke<TicketType[]>('get_tickets', { boardId: ticket.boardId });
            const parent = tickets.find(t => t.id === ticket.epicId);
            setParentEpic(parent || null);
          } catch (e) {
            logger.error('Failed to load parent epic:', e);
          }
          setEpicChildren([]);
          setEpicProgress(null);
          setAvailableTickets([]);
        } else {
          // Not epic-related
          setEpicChildren([]);
          setEpicProgress(null);
          setParentEpic(null);
          setAvailableTickets([]);
        }
      } catch (e) {
        logger.error('Failed to load epic data:', e);
      } finally {
        setLoadingEpic(false);
      }
    };
    loadEpicData();
  }, [ticket.id, ticket.isEpic, ticket.epicId, ticket.boardId]);

  const handleAddChild = useCallback(async () => {
    if (!selectedChildId) return;
    setIsAddingChild(true);
    try {
      await invoke('add_ticket_to_epic', { epicId: ticket.id, ticketId: selectedChildId });
      // Refresh epic data
      const [children, progress] = await Promise.all([
        invoke<TicketType[]>('get_epic_children', { epicId: ticket.id }),
        invoke<EpicProgress>('get_epic_progress', { epicId: ticket.id }),
      ]);
      setEpicChildren(children);
      setEpicProgress(progress);
      // Remove from available tickets
      setAvailableTickets(prev => prev.filter(t => t.id !== selectedChildId));
      setSelectedChildId('');
    } catch (e) {
      logger.error('Failed to add child to epic:', e);
    } finally {
      setIsAddingChild(false);
    }
  }, [ticket.id, selectedChildId]);

  const handleRemoveChild = useCallback(async (childId: string) => {
    try {
      await invoke('remove_ticket_from_epic', { ticketId: childId });
      // Refresh epic data
      const [children, progress, allTickets] = await Promise.all([
        invoke<TicketType[]>('get_epic_children', { epicId: ticket.id }),
        invoke<EpicProgress>('get_epic_progress', { epicId: ticket.id }),
        invoke<TicketType[]>('get_tickets', { boardId: ticket.boardId }),
      ]);
      setEpicChildren(children);
      setEpicProgress(progress);
      // Refresh available tickets
      const available = allTickets.filter(t => 
        !t.isEpic && 
        !t.epicId && 
        t.id !== ticket.id
      );
      setAvailableTickets(available);
    } catch (e) {
      logger.error('Failed to remove child from epic:', e);
    }
  }, [ticket.id, ticket.boardId]);

  const handleMoveChild = useCallback(async (childIndex: number, direction: 'up' | 'down') => {
    if (direction === 'up' && childIndex === 0) return;
    if (direction === 'down' && childIndex === epicChildren.length - 1) return;
    
    const newChildren = [...epicChildren];
    const targetIndex = direction === 'up' ? childIndex - 1 : childIndex + 1;
    
    // Swap the children
    [newChildren[childIndex], newChildren[targetIndex]] = [newChildren[targetIndex], newChildren[childIndex]];
    
    // Optimistically update UI
    setEpicChildren(newChildren);
    
    try {
      // Persist the new order
      const childIds = newChildren.map(c => c.id);
      await invoke('reorder_epic_children', { epicId: ticket.id, childIds });
    } catch (e) {
      logger.error('Failed to reorder children:', e);
      // Revert on error
      setEpicChildren(epicChildren);
    }
  }, [ticket.id, epicChildren]);

  return {
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
  };
}
