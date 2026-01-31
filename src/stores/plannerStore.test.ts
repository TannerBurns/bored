import { describe, it, expect, beforeEach, vi } from 'vitest';
import { usePlannerStore } from './plannerStore';
import type { Scratchpad } from '../types';

// Mock @tauri-apps/api/tauri
vi.mock('@tauri-apps/api/tauri', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/tauri';

const mockScratchpad: Scratchpad = {
  id: 'scratch-1',
  boardId: 'board-1',
  targetBoardId: 'board-1',
  projectId: 'project-1',
  name: 'Test Scratchpad',
  userInput: 'Build a feature',
  status: 'draft',
  agentPref: 'claude',
  model: 'opus',
  explorationLog: [],
  planMarkdown: undefined,
  planJson: undefined,
  settings: {},
  createdAt: new Date('2024-01-01'),
  updatedAt: new Date('2024-01-01'),
};

describe('usePlannerStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    usePlannerStore.setState({
      scratchpads: [],
      currentScratchpad: null,
      scratchpadTickets: [],
      isLoading: false,
      isExploring: false,
      isPlanning: false,
      error: null,
    });
  });

  describe('loadScratchpads', () => {
    it('loads scratchpads for a board', async () => {
      vi.mocked(invoke).mockResolvedValueOnce([mockScratchpad]);

      await usePlannerStore.getState().loadScratchpads('board-1');

      expect(invoke).toHaveBeenCalledWith('get_scratchpads', { boardId: 'board-1' });
      expect(usePlannerStore.getState().scratchpads).toHaveLength(1);
      expect(usePlannerStore.getState().scratchpads[0].id).toBe('scratch-1');
      expect(usePlannerStore.getState().isLoading).toBe(false);
    });

    it('sets error on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Network error'));

      await usePlannerStore.getState().loadScratchpads('board-1');

      expect(usePlannerStore.getState().error).toBe('Error: Network error');
      expect(usePlannerStore.getState().isLoading).toBe(false);
    });
  });

  describe('getScratchpad', () => {
    it('fetches a single scratchpad', async () => {
      vi.mocked(invoke).mockResolvedValueOnce(mockScratchpad);

      const result = await usePlannerStore.getState().getScratchpad('scratch-1');

      expect(invoke).toHaveBeenCalledWith('get_scratchpad', { id: 'scratch-1' });
      expect(result.id).toBe('scratch-1');
    });

    it('throws on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Not found'));

      await expect(
        usePlannerStore.getState().getScratchpad('nonexistent')
      ).rejects.toThrow('Not found');
    });
  });

  describe('createScratchpad', () => {
    it('creates and adds scratchpad to state', async () => {
      vi.mocked(invoke).mockResolvedValueOnce(mockScratchpad);

      const result = await usePlannerStore.getState().createScratchpad({
        boardId: 'board-1',
        projectId: 'project-1',
        name: 'Test Scratchpad',
        userInput: 'Build a feature',
      });

      expect(invoke).toHaveBeenCalledWith('create_scratchpad', {
        input: expect.objectContaining({
          boardId: 'board-1',
          projectId: 'project-1',
          name: 'Test Scratchpad',
          userInput: 'Build a feature',
        }),
      });
      expect(result.id).toBe('scratch-1');
      expect(usePlannerStore.getState().scratchpads).toHaveLength(1);
      expect(usePlannerStore.getState().currentScratchpad?.id).toBe('scratch-1');
    });

    it('sets error on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Creation failed'));

      await expect(
        usePlannerStore.getState().createScratchpad({
          boardId: 'board-1',
          projectId: 'project-1',
          name: 'Test',
          userInput: 'Input',
        })
      ).rejects.toThrow('Creation failed');

      expect(usePlannerStore.getState().error).toBe('Error: Creation failed');
    });
  });

  describe('deleteScratchpad', () => {
    it('removes scratchpad from state', async () => {
      usePlannerStore.setState({
        scratchpads: [mockScratchpad],
        currentScratchpad: mockScratchpad,
      });
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      await usePlannerStore.getState().deleteScratchpad('scratch-1');

      expect(invoke).toHaveBeenCalledWith('delete_scratchpad', { id: 'scratch-1' });
      expect(usePlannerStore.getState().scratchpads).toHaveLength(0);
      expect(usePlannerStore.getState().currentScratchpad).toBeNull();
    });
  });

  describe('selectScratchpad', () => {
    it('sets current scratchpad', () => {
      usePlannerStore.getState().selectScratchpad(mockScratchpad);

      expect(usePlannerStore.getState().currentScratchpad?.id).toBe('scratch-1');
    });

    it('clears current scratchpad when null', () => {
      usePlannerStore.setState({ currentScratchpad: mockScratchpad });

      usePlannerStore.getState().selectScratchpad(null);

      expect(usePlannerStore.getState().currentScratchpad).toBeNull();
    });
  });

  describe('approvePlan', () => {
    it('approves plan and refreshes scratchpad', async () => {
      const approvedScratchpad = { ...mockScratchpad, status: 'approved' as const };
      usePlannerStore.setState({
        scratchpads: [mockScratchpad],
        currentScratchpad: mockScratchpad,
      });
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // approve_plan
        .mockResolvedValueOnce(approvedScratchpad); // get_scratchpad refresh

      await usePlannerStore.getState().approvePlan('scratch-1');

      expect(invoke).toHaveBeenCalledWith('approve_plan', { id: 'scratch-1' });
      expect(usePlannerStore.getState().currentScratchpad?.status).toBe('approved');
    });
  });

  describe('state setters', () => {
    it('setScratchpads updates scratchpads', () => {
      usePlannerStore.getState().setScratchpads([mockScratchpad]);
      expect(usePlannerStore.getState().scratchpads).toHaveLength(1);
    });

    it('setCurrentScratchpad updates current', () => {
      usePlannerStore.getState().setCurrentScratchpad(mockScratchpad);
      expect(usePlannerStore.getState().currentScratchpad?.id).toBe('scratch-1');
    });

    it('setLoading updates loading state', () => {
      usePlannerStore.getState().setLoading(true);
      expect(usePlannerStore.getState().isLoading).toBe(true);
    });

    it('setExploring updates exploring state', () => {
      usePlannerStore.getState().setExploring(true);
      expect(usePlannerStore.getState().isExploring).toBe(true);
    });

    it('setPlanning updates planning state', () => {
      usePlannerStore.getState().setPlanning(true);
      expect(usePlannerStore.getState().isPlanning).toBe(true);
    });

    it('setError updates error state', () => {
      usePlannerStore.getState().setError('Test error');
      expect(usePlannerStore.getState().error).toBe('Test error');
    });
  });
});
