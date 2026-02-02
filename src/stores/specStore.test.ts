import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useSpecStore } from './specStore';
import type { Spec } from '../types';

// Mock @tauri-apps/api/tauri
vi.mock('@tauri-apps/api/tauri', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/tauri';

const mockSpec: Spec = {
  id: 'scratch-1',
  boardId: 'board-1',
  targetBoardId: 'board-1',
  projectId: 'project-1',
  name: 'Test Spec',
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

describe('useSpecStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSpecStore.setState({
      specs: [],
      currentSpec: null,
      specTickets: [],
      liveLogs: [],
      currentEta: null,
      isLoading: false,
      isExploring: false,
      isPlanning: false,
      error: null,
    });
  });

  describe('loadSpecs', () => {
    it('loads specs for a board', async () => {
      vi.mocked(invoke).mockResolvedValueOnce([mockSpec]);

      await useSpecStore.getState().loadSpecs('board-1');

      expect(invoke).toHaveBeenCalledWith('get_specs', { boardId: 'board-1' });
      expect(useSpecStore.getState().specs).toHaveLength(1);
      expect(useSpecStore.getState().specs[0].id).toBe('scratch-1');
      expect(useSpecStore.getState().isLoading).toBe(false);
    });

    it('sets error on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Network error'));

      await useSpecStore.getState().loadSpecs('board-1');

      expect(useSpecStore.getState().error).toBe('Error: Network error');
      expect(useSpecStore.getState().isLoading).toBe(false);
    });
  });

  describe('getSpec', () => {
    it('fetches a single spec', async () => {
      vi.mocked(invoke).mockResolvedValueOnce(mockSpec);

      const result = await useSpecStore.getState().getSpec('scratch-1');

      expect(invoke).toHaveBeenCalledWith('get_spec', { id: 'scratch-1' });
      expect(result.id).toBe('scratch-1');
    });

    it('throws on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Not found'));

      await expect(
        useSpecStore.getState().getSpec('nonexistent')
      ).rejects.toThrow('Not found');
    });
  });

  describe('createSpec', () => {
    it('creates and adds spec to state', async () => {
      vi.mocked(invoke).mockResolvedValueOnce(mockSpec);

      const result = await useSpecStore.getState().createSpec({
        boardId: 'board-1',
        projectId: 'project-1',
        name: 'Test Spec',
        userInput: 'Build a feature',
      });

      expect(invoke).toHaveBeenCalledWith('create_spec', {
        input: expect.objectContaining({
          boardId: 'board-1',
          projectId: 'project-1',
          name: 'Test Spec',
          userInput: 'Build a feature',
        }),
      });
      expect(result.id).toBe('scratch-1');
      expect(useSpecStore.getState().specs).toHaveLength(1);
      expect(useSpecStore.getState().currentSpec?.id).toBe('scratch-1');
    });

    it('sets error on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Creation failed'));

      await expect(
        useSpecStore.getState().createSpec({
          boardId: 'board-1',
          projectId: 'project-1',
          name: 'Test',
          userInput: 'Input',
        })
      ).rejects.toThrow('Creation failed');

      expect(useSpecStore.getState().error).toBe('Error: Creation failed');
    });
  });

  describe('deleteSpec', () => {
    it('removes spec from state', async () => {
      useSpecStore.setState({
        specs: [mockSpec],
        currentSpec: mockSpec,
      });
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      await useSpecStore.getState().deleteSpec('scratch-1');

      expect(invoke).toHaveBeenCalledWith('delete_spec', { id: 'scratch-1' });
      expect(useSpecStore.getState().specs).toHaveLength(0);
      expect(useSpecStore.getState().currentSpec).toBeNull();
    });
  });

  describe('selectSpec', () => {
    it('sets current spec', () => {
      useSpecStore.getState().selectSpec(mockSpec);

      expect(useSpecStore.getState().currentSpec?.id).toBe('scratch-1');
    });

    it('clears current spec when null', () => {
      useSpecStore.setState({ currentSpec: mockSpec });

      useSpecStore.getState().selectSpec(null);

      expect(useSpecStore.getState().currentSpec).toBeNull();
    });
  });

  describe('approvePlan', () => {
    it('approves plan and refreshes spec', async () => {
      const approvedSpec = { ...mockSpec, status: 'approved' as const };
      useSpecStore.setState({
        specs: [mockSpec],
        currentSpec: mockSpec,
      });
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // approve_plan
        .mockResolvedValueOnce(approvedSpec); // get_spec refresh

      await useSpecStore.getState().approvePlan('scratch-1');

      expect(invoke).toHaveBeenCalledWith('approve_plan', { id: 'scratch-1' });
      expect(useSpecStore.getState().currentSpec?.status).toBe('approved');
    });
  });

  describe('pauseWork', () => {
    it('pauses work and refreshes state', async () => {
      const pausedSpec = { ...mockSpec, status: 'paused' as const };
      useSpecStore.setState({
        specs: [mockSpec],
        currentSpec: mockSpec,
      });
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // pause_spec_work
        .mockResolvedValueOnce(pausedSpec); // get_spec refresh

      await useSpecStore.getState().pauseWork('scratch-1');

      expect(invoke).toHaveBeenCalledWith('pause_spec_work', { specId: 'scratch-1' });
      expect(useSpecStore.getState().currentSpec?.status).toBe('paused');
    });

    it('throws on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Cannot pause'));

      await expect(
        useSpecStore.getState().pauseWork('scratch-1')
      ).rejects.toThrow('Cannot pause');
    });
  });

  describe('resumeWork', () => {
    it('resumes work and refreshes state', async () => {
      const pausedSpec = { ...mockSpec, status: 'paused' as const };
      const workingSpec = { ...mockSpec, status: 'working' as const };
      useSpecStore.setState({
        specs: [pausedSpec],
        currentSpec: pausedSpec,
      });
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // resume_spec_work
        .mockResolvedValueOnce(workingSpec); // get_spec refresh

      await useSpecStore.getState().resumeWork('scratch-1');

      expect(invoke).toHaveBeenCalledWith('resume_spec_work', { specId: 'scratch-1' });
      expect(useSpecStore.getState().currentSpec?.status).toBe('working');
    });

    it('throws on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Cannot resume'));

      await expect(
        useSpecStore.getState().resumeWork('scratch-1')
      ).rejects.toThrow('Cannot resume');
    });
  });

  describe('haltWork', () => {
    it('halts work and refreshes state', async () => {
      const workingSpec = { ...mockSpec, status: 'working' as const };
      const haltedSpec = { ...mockSpec, status: 'halted' as const };
      useSpecStore.setState({
        specs: [workingSpec],
        currentSpec: workingSpec,
      });
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // halt_spec_work
        .mockResolvedValueOnce(haltedSpec); // get_spec refresh

      await useSpecStore.getState().haltWork('scratch-1');

      expect(invoke).toHaveBeenCalledWith('halt_spec_work', { specId: 'scratch-1' });
      expect(useSpecStore.getState().currentSpec?.status).toBe('halted');
    });

    it('throws on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Cannot halt'));

      await expect(
        useSpecStore.getState().haltWork('scratch-1')
      ).rejects.toThrow('Cannot halt');
    });
  });

  describe('loadEta', () => {
    it('loads ETA and updates state', async () => {
      const mockEta = {
        specId: 'scratch-1',
        totalTickets: 10,
        completedTickets: 5,
        inProgressTickets: 2,
        pausedTickets: 0,
        elapsedSeconds: 300,
        estimatedSecondsRemaining: 300,
        confidence: 'medium' as const,
        avgSecondsPerStage: {},
      };
      vi.mocked(invoke).mockResolvedValueOnce(mockEta);

      await useSpecStore.getState().loadEta('scratch-1');

      expect(invoke).toHaveBeenCalledWith('get_spec_eta', { specId: 'scratch-1' });
      expect(useSpecStore.getState().currentEta).toEqual(mockEta);
    });

    it('sets null on failure without throwing', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('ETA failed'));

      // Should not throw
      await useSpecStore.getState().loadEta('scratch-1');

      expect(useSpecStore.getState().currentEta).toBeNull();
    });
  });

  describe('loadSpecTickets', () => {
    it('loads tickets for a spec', async () => {
      const mockTickets = [
        { id: 'ticket-1', title: 'Task 1' },
        { id: 'ticket-2', title: 'Task 2' },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockTickets);

      await useSpecStore.getState().loadSpecTickets('scratch-1');

      expect(invoke).toHaveBeenCalledWith('get_spec_tickets', { id: 'scratch-1' });
      expect(useSpecStore.getState().specTickets).toHaveLength(2);
    });

    it('throws on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Load failed'));

      await expect(
        useSpecStore.getState().loadSpecTickets('scratch-1')
      ).rejects.toThrow('Load failed');
    });
  });

  describe('state setters', () => {
    it('setSpecs updates specs', () => {
      useSpecStore.getState().setSpecs([mockSpec]);
      expect(useSpecStore.getState().specs).toHaveLength(1);
    });

    it('setCurrentSpec updates current', () => {
      useSpecStore.getState().setCurrentSpec(mockSpec);
      expect(useSpecStore.getState().currentSpec?.id).toBe('scratch-1');
    });

    it('setLoading updates loading state', () => {
      useSpecStore.getState().setLoading(true);
      expect(useSpecStore.getState().isLoading).toBe(true);
    });

    it('setExploring updates exploring state', () => {
      useSpecStore.getState().setExploring(true);
      expect(useSpecStore.getState().isExploring).toBe(true);
    });

    it('setPlanning updates planning state', () => {
      useSpecStore.getState().setPlanning(true);
      expect(useSpecStore.getState().isPlanning).toBe(true);
    });

    it('setError updates error state', () => {
      useSpecStore.getState().setError('Test error');
      expect(useSpecStore.getState().error).toBe('Test error');
    });
  });
});
