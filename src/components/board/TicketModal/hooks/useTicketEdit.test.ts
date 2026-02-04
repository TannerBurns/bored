import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useTicketEdit, type UseTicketEditOptions } from './useTicketEdit';
import type { Ticket } from '../../../../types';

type OnUpdateFn = UseTicketEditOptions['onUpdate'];

const createMockTicket = (overrides: Partial<Ticket> = {}): Ticket => ({
  id: 'ticket-1',
  boardId: 'board-1',
  columnId: 'col-1',
  title: 'Test Ticket',
  descriptionMd: 'Test description',
  priority: 'medium',
  labels: ['bug', 'frontend'],
  createdAt: new Date('2024-01-01'),
  updatedAt: new Date('2024-01-01'),
  projectId: 'proj-1',
  model: 'sonnet-4',
  branchName: 'feat/test',
  ...overrides,
});

describe('useTicketEdit', () => {
  let mockOnUpdate: Mock<OnUpdateFn>;

  beforeEach(() => {
    mockOnUpdate = vi.fn<OnUpdateFn>(() => Promise.resolve());
  });

  describe('initialization', () => {
    it('initializes with ticket values', () => {
      const ticket = createMockTicket();
      const { result } = renderHook(() =>
        useTicketEdit({ ticket, onUpdate: mockOnUpdate })
      );

      expect(result.current.isEditing).toBe(false);
      expect(result.current.editTitle).toBe('Test Ticket');
      expect(result.current.editDescription).toBe('Test description');
      expect(result.current.editPriority).toBe('medium');
      expect(result.current.editLabels).toBe('bug, frontend');
      expect(result.current.editProjectId).toBe('proj-1');
      expect(result.current.editModel).toBe('sonnet-4');
      expect(result.current.editBranchName).toBe('feat/test');
      expect(result.current.editColumnId).toBe('col-1');
      expect(result.current.isSaving).toBe(false);
    });

    it('defaults optional fields to empty string', () => {
      const ticket = createMockTicket({
        projectId: undefined,
        model: undefined,
        branchName: undefined,
      });
      const { result } = renderHook(() =>
        useTicketEdit({ ticket, onUpdate: mockOnUpdate })
      );

      expect(result.current.editProjectId).toBe('');
      expect(result.current.editModel).toBe('');
      expect(result.current.editBranchName).toBe('');
    });
  });

  describe('state reset on ticket change', () => {
    it('resets state when ticket.id changes', () => {
      const ticket1 = createMockTicket({ id: 'ticket-1', title: 'First' });
      const ticket2 = createMockTicket({ id: 'ticket-2', title: 'Second' });

      const { result, rerender } = renderHook(
        ({ ticket }) => useTicketEdit({ ticket, onUpdate: mockOnUpdate }),
        { initialProps: { ticket: ticket1 } }
      );

      act(() => {
        result.current.setEditTitle('Modified');
        result.current.setIsEditing(true);
      });

      expect(result.current.editTitle).toBe('Modified');
      expect(result.current.isEditing).toBe(true);

      rerender({ ticket: ticket2 });

      expect(result.current.editTitle).toBe('Second');
      expect(result.current.isEditing).toBe(false);
    });

    it('resets state when ticket.pausedAt changes', () => {
      const ticket1 = createMockTicket({ pausedAt: undefined });
      const ticket2 = createMockTicket({ pausedAt: new Date() });

      const { result, rerender } = renderHook(
        ({ ticket }) => useTicketEdit({ ticket, onUpdate: mockOnUpdate }),
        { initialProps: { ticket: ticket1 } }
      );

      act(() => {
        result.current.setEditTitle('Modified');
        result.current.setIsEditing(true);
      });

      rerender({ ticket: ticket2 });

      expect(result.current.editTitle).toBe('Test Ticket');
      expect(result.current.isEditing).toBe(false);
    });
  });

  describe('resetEditState', () => {
    it('restores original ticket values', () => {
      const ticket = createMockTicket();
      const { result } = renderHook(() =>
        useTicketEdit({ ticket, onUpdate: mockOnUpdate })
      );

      act(() => {
        result.current.setEditTitle('Changed Title');
        result.current.setEditDescription('Changed Desc');
        result.current.setEditPriority('urgent');
        result.current.setEditLabels('new, labels');
      });

      expect(result.current.editTitle).toBe('Changed Title');

      act(() => {
        result.current.resetEditState();
      });

      expect(result.current.editTitle).toBe('Test Ticket');
      expect(result.current.editDescription).toBe('Test description');
      expect(result.current.editPriority).toBe('medium');
      expect(result.current.editLabels).toBe('bug, frontend');
    });
  });

  describe('handleSave', () => {
    it('calls onUpdate with correct data', async () => {
      const ticket = createMockTicket();
      const { result } = renderHook(() =>
        useTicketEdit({ ticket, onUpdate: mockOnUpdate })
      );

      await act(async () => {
        await result.current.handleSave();
      });

      expect(mockOnUpdate).toHaveBeenCalledWith('ticket-1', {
        title: 'Test Ticket',
        descriptionMd: 'Test description',
        priority: 'medium',
        labels: ['bug', 'frontend'],
        projectId: 'proj-1',
        workflowType: 'multi_stage',
        model: 'sonnet-4',
        branchName: 'feat/test',
        columnId: 'col-1',
      });
    });

    it('parses labels correctly', async () => {
      const ticket = createMockTicket({ labels: [] });
      const { result } = renderHook(() =>
        useTicketEdit({ ticket, onUpdate: mockOnUpdate })
      );

      act(() => {
        result.current.setEditLabels('  one  ,  two  , , three  ');
      });

      await act(async () => {
        await result.current.handleSave();
      });

      expect(mockOnUpdate).toHaveBeenCalledWith(
        'ticket-1',
        expect.objectContaining({
          labels: ['one', 'two', 'three'],
        })
      );
    });

    it('sets model to undefined when empty', async () => {
      const ticket = createMockTicket({ model: undefined });
      const { result } = renderHook(() =>
        useTicketEdit({ ticket, onUpdate: mockOnUpdate })
      );

      await act(async () => {
        await result.current.handleSave();
      });

      expect(mockOnUpdate).toHaveBeenCalledWith(
        'ticket-1',
        expect.objectContaining({
          model: undefined,
        })
      );
    });

    it('sets isSaving during save', async () => {
      let resolveSave: () => void;
      const slowUpdate = vi.fn(
        () => new Promise<void>((resolve) => (resolveSave = resolve))
      );

      const ticket = createMockTicket();
      const { result } = renderHook(() =>
        useTicketEdit({ ticket, onUpdate: slowUpdate })
      );

      expect(result.current.isSaving).toBe(false);

      let savePromise: Promise<void>;
      act(() => {
        savePromise = result.current.handleSave();
      });

      expect(result.current.isSaving).toBe(true);

      await act(async () => {
        resolveSave!();
        await savePromise;
      });

      expect(result.current.isSaving).toBe(false);
    });

    it('sets isEditing to false after save', async () => {
      const ticket = createMockTicket();
      const { result } = renderHook(() =>
        useTicketEdit({ ticket, onUpdate: mockOnUpdate })
      );

      act(() => {
        result.current.setIsEditing(true);
      });

      expect(result.current.isEditing).toBe(true);

      await act(async () => {
        await result.current.handleSave();
      });

      expect(result.current.isEditing).toBe(false);
    });

    it('resets isSaving even on error', async () => {
      const failingUpdate = vi.fn(() => Promise.reject(new Error('fail')));
      const ticket = createMockTicket();
      const { result } = renderHook(() =>
        useTicketEdit({ ticket, onUpdate: failingUpdate })
      );

      await act(async () => {
        try {
          await result.current.handleSave();
        } catch {
          // Expected
        }
      });

      expect(result.current.isSaving).toBe(false);
    });
  });
});
