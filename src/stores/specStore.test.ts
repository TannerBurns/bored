import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useSpecStore } from './specStore';
import type { SpecWithVersion, SpecVersion } from '../types';

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';

const mockVersion: SpecVersion = {
  id: 'version-1',
  specId: 'scratch-1',
  versionNumber: 1,
  status: 'conversing',
  explorationLog: [],
  planMarkdown: undefined,
  planJson: undefined,
  workStartedAt: undefined,
  createdAt: new Date('2024-01-01'),
  updatedAt: new Date('2024-01-01'),
};

const mockSpec: SpecWithVersion = {
  id: 'scratch-1',
  boardId: 'board-1',
  targetBoardId: 'board-1',
  projectId: 'project-1',
  name: 'Test Spec',
  userInput: 'Build a feature',
  model: 'opus',
  settings: {},
  createdAt: new Date('2024-01-01'),
  updatedAt: new Date('2024-01-01'),
  latestVersion: mockVersion,
  versionCount: 1,
};

describe('useSpecStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSpecStore.setState({
      specs: [],
      currentSpec: null,
      currentVersions: [],
      selectedVersion: null,
      selectedVersionId: null,
      activeTab: 'chat',
      scrollToProgress: false,
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

      expect(invoke).toHaveBeenCalledWith('get_specs_with_versions', { boardId: 'board-1' });
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
    it('fetches a single spec with version', async () => {
      vi.mocked(invoke).mockResolvedValueOnce(mockSpec);

      const result = await useSpecStore.getState().getSpec('scratch-1');

      expect(invoke).toHaveBeenCalledWith('get_spec_with_version', { id: 'scratch-1' });
      expect(result.id).toBe('scratch-1');
      expect(result.latestVersion?.status).toBe('conversing');
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
      // Mock the base spec response (from create_spec) and full spec response (from get_spec_with_version)
      const baseSpec = { id: mockSpec.id, boardId: mockSpec.boardId, projectId: mockSpec.projectId, name: mockSpec.name, userInput: mockSpec.userInput, settings: {} };
      vi.mocked(invoke)
        .mockResolvedValueOnce(baseSpec) // create_spec
        .mockResolvedValueOnce(mockSpec); // get_spec_with_version

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
      expect(useSpecStore.getState().currentSpec?.latestVersion?.status).toBe('conversing');
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

  describe('selectSpecForProgress', () => {
    it('selects spec, switches to versions tab, and enables scroll', () => {
      useSpecStore.getState().selectSpecForProgress(mockSpec);

      const state = useSpecStore.getState();
      expect(state.currentSpec?.id).toBe('scratch-1');
      expect(state.activeTab).toBe('versions');
      expect(state.selectedVersion?.id).toBe('version-1');
      expect(state.selectedVersionId).toBe('version-1');
      expect(state.currentVersions).toHaveLength(1);
      expect(state.currentVersions[0].id).toBe('version-1');
      expect(state.scrollToProgress).toBe(true);
    });

    it('always targets the latest version even if another was selected', () => {
      const otherVersion: SpecVersion = { ...mockVersion, id: 'version-old', versionNumber: 0 };
      useSpecStore.setState({ selectedVersion: otherVersion, selectedVersionId: 'version-old' });

      useSpecStore.getState().selectSpecForProgress(mockSpec);

      expect(useSpecStore.getState().selectedVersion?.id).toBe('version-1');
      expect(useSpecStore.getState().selectedVersionId).toBe('version-1');
    });

    it('handles spec with no latestVersion', () => {
      const specNoVersion: SpecWithVersion = { ...mockSpec, latestVersion: undefined };

      useSpecStore.getState().selectSpecForProgress(specNoVersion);

      const state = useSpecStore.getState();
      expect(state.currentSpec?.id).toBe('scratch-1');
      expect(state.activeTab).toBe('versions');
      expect(state.selectedVersion).toBeNull();
      expect(state.selectedVersionId).toBeNull();
      expect(state.currentVersions).toHaveLength(0);
      expect(state.scrollToProgress).toBe(true);
    });

    it('differs from selectSpec by setting versions tab instead of chat', () => {
      useSpecStore.getState().selectSpec(mockSpec);
      expect(useSpecStore.getState().activeTab).toBe('chat');

      useSpecStore.getState().selectSpecForProgress(mockSpec);
      expect(useSpecStore.getState().activeTab).toBe('versions');
    });
  });

  describe('setScrollToProgress', () => {
    it('sets scrollToProgress to true', () => {
      useSpecStore.getState().setScrollToProgress(true);
      expect(useSpecStore.getState().scrollToProgress).toBe(true);
    });

    it('sets scrollToProgress back to false', () => {
      useSpecStore.setState({ scrollToProgress: true });
      useSpecStore.getState().setScrollToProgress(false);
      expect(useSpecStore.getState().scrollToProgress).toBe(false);
    });
  });

  describe('scrollToProgress initial state', () => {
    it('defaults to false', () => {
      expect(useSpecStore.getState().scrollToProgress).toBe(false);
    });
  });

  describe('approvePlan', () => {
    it('approves plan and refreshes spec', async () => {
      const approvedVersion = { ...mockVersion, status: 'approved' as const };
      const approvedSpec = { ...mockSpec, latestVersion: approvedVersion };
      useSpecStore.setState({
        specs: [mockSpec],
        currentSpec: mockSpec,
      });
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // approve_plan
        .mockResolvedValueOnce(approvedSpec); // get_spec_with_version refresh

      await useSpecStore.getState().approvePlan('scratch-1');

      expect(invoke).toHaveBeenCalledWith('approve_plan', { id: 'scratch-1' });
      expect(useSpecStore.getState().currentSpec?.latestVersion?.status).toBe('approved');
    });
  });

  describe('pauseWork', () => {
    it('pauses work and refreshes state', async () => {
      const pausedVersion = { ...mockVersion, status: 'paused' as const };
      const pausedSpec = { ...mockSpec, latestVersion: pausedVersion };
      useSpecStore.setState({
        specs: [mockSpec],
        currentSpec: mockSpec,
      });
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // pause_spec_work
        .mockResolvedValueOnce(pausedSpec); // get_spec_with_version refresh

      await useSpecStore.getState().pauseWork('scratch-1');

      expect(invoke).toHaveBeenCalledWith('pause_spec_work', { specId: 'scratch-1' });
      expect(useSpecStore.getState().currentSpec?.latestVersion?.status).toBe('paused');
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
      const pausedVersion = { ...mockVersion, status: 'paused' as const };
      const workingVersion = { ...mockVersion, status: 'working' as const };
      const pausedSpec = { ...mockSpec, latestVersion: pausedVersion };
      const workingSpec = { ...mockSpec, latestVersion: workingVersion };
      useSpecStore.setState({
        specs: [pausedSpec],
        currentSpec: pausedSpec,
      });
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // resume_spec_work
        .mockResolvedValueOnce(workingSpec); // get_spec_with_version refresh

      await useSpecStore.getState().resumeWork('scratch-1');

      expect(invoke).toHaveBeenCalledWith('resume_spec_work', { specId: 'scratch-1' });
      expect(useSpecStore.getState().currentSpec?.latestVersion?.status).toBe('working');
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
      const workingVersion = { ...mockVersion, status: 'working' as const };
      const haltedVersion = { ...mockVersion, status: 'halted' as const };
      const workingSpec = { ...mockSpec, latestVersion: workingVersion };
      const haltedSpec = { ...mockSpec, latestVersion: haltedVersion };
      useSpecStore.setState({
        specs: [workingSpec],
        currentSpec: workingSpec,
      });
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // halt_spec_work
        .mockResolvedValueOnce(haltedSpec); // get_spec_with_version refresh

      await useSpecStore.getState().haltWork('scratch-1');

      expect(invoke).toHaveBeenCalledWith('halt_spec_work', { specId: 'scratch-1' });
      expect(useSpecStore.getState().currentSpec?.latestVersion?.status).toBe('halted');
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

  describe('addBrainstormLog', () => {
    it('adds a log message', () => {
      useSpecStore.getState().addBrainstormLog('first');
      expect(useSpecStore.getState().brainstormLogs).toEqual(['first']);
    });

    it('caps logs at 4 entries for rolling visual effect', () => {
      const store = useSpecStore.getState();
      store.addBrainstormLog('msg-1');
      store.addBrainstormLog('msg-2');
      store.addBrainstormLog('msg-3');
      store.addBrainstormLog('msg-4');
      store.addBrainstormLog('msg-5');

      const logs = useSpecStore.getState().brainstormLogs;
      expect(logs).toHaveLength(4);
      expect(logs[0]).toBe('msg-2');
      expect(logs[3]).toBe('msg-5');
    });

    it('clearBrainstormLogs empties the array', () => {
      useSpecStore.getState().addBrainstormLog('entry');
      useSpecStore.getState().clearBrainstormLogs();
      expect(useSpecStore.getState().brainstormLogs).toEqual([]);
    });
  });
});
