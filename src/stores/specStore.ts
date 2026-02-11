import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { Spec, SpecVersion, SpecWithVersion, CreateSpecInput, UpdateSpecInput, Ticket, SpecEta, ConversationMessage } from '../types';
import { logger } from '../lib/logger';

/** A single log entry from the planner agent */
export interface SpecLogEntry {
  id: string;
  specId: string;
  phase: 'exploration' | 'planning';
  level: 'info' | 'output' | 'error';
  message: string;
  timestamp: string;
}

interface SpecState {
  specs: SpecWithVersion[];
  currentSpec: SpecWithVersion | null;
  /** All versions of the current spec */
  currentVersions: SpecVersion[];
  /** The selected version of the current spec */
  selectedVersion: SpecVersion | null;
  /** ID of the selected version (for tab linking) */
  selectedVersionId: string | null;
  /** Currently active tab in the spec detail view */
  activeTab: 'chat' | 'versions';
  specTickets: Ticket[];
  /** Real-time log entries from agent output */
  liveLogs: SpecLogEntry[];
  /** ETA information for the current spec */
  currentEta: SpecEta | null;
  /** Conversation messages for current spec brainstorming */
  conversationMessages: ConversationMessage[];
  /** Whether the agent is currently thinking/responding */
  isAgentThinking: boolean;
  /** Real-time log entries from brainstorm agent */
  brainstormLogs: string[];
  /** Whether the spec is being generated (no more questions) */
  isGeneratingSpec: boolean;
  /** Version number being generated */
  generatingVersionNumber: number | null;
  /** When true, VersionDetail should auto-scroll to the EpicProgressPanel */
  scrollToProgress: boolean;
  isLoading: boolean;
  isExploring: boolean;
  isPlanning: boolean;
  error: string | null;

  // Actions
  loadSpecs: (boardId: string) => Promise<void>;
  loadAllSpecs: () => Promise<void>;
  getSpec: (id: string) => Promise<SpecWithVersion>;
  createSpec: (input: CreateSpecInput) => Promise<Spec>;
  updateSpec: (id: string, updates: UpdateSpecInput) => Promise<Spec>;
  deleteSpec: (id: string, deleteTickets?: boolean) => Promise<void>;
  selectSpec: (spec: SpecWithVersion | null) => void;
  /** Select a spec, switch to versions tab, and scroll to progress (targets latest version) */
  selectSpecForProgress: (spec: SpecWithVersion) => void;
  setScrollToProgress: (scroll: boolean) => void;
  
  // Version management
  loadVersions: (specId: string) => Promise<void>;
  selectVersion: (version: SpecVersion | null) => void;
  selectVersionById: (versionId: string) => void;
  setActiveTab: (tab: 'chat' | 'versions') => void;
  createNewVersion: (specId: string) => Promise<SpecVersion>;
  
  // Status management (operates on latest version)
  setStatus: (id: string, status: string) => Promise<void>;
  
  // Exploration (operates on latest version)
  appendExploration: (id: string, query: string, response: string) => Promise<void>;
  
  // Plan management (operates on latest version)
  setPlan: (id: string, markdown: string, json?: unknown) => Promise<void>;
  approvePlan: (id: string) => Promise<void>;
  
  // Get tickets created from spec (latest version)
  loadSpecTickets: (id: string) => Promise<void>;
  
  // Pause/Resume/Halt controls (operates on latest version)
  pauseWork: (id: string) => Promise<void>;
  resumeWork: (id: string) => Promise<void>;
  haltWork: (id: string) => Promise<void>;
  
  // ETA
  loadEta: (id: string) => Promise<void>;
  
  // Live log management
  addLogEntry: (entry: Omit<SpecLogEntry, 'id'>) => void;
  clearLogs: (specId?: string) => void;
  
  // Conversation management
  setConversationMessages: (messages: ConversationMessage[]) => void;
  addConversationMessage: (message: ConversationMessage) => void;
  setAgentThinking: (thinking: boolean) => void;
  clearConversation: () => void;
  
  // Brainstorm log management
  addBrainstormLog: (message: string) => void;
  clearBrainstormLogs: () => void;
  setGeneratingSpec: (generating: boolean, versionNumber?: number) => void;
  
  // State setters
  setSpecs: (specs: SpecWithVersion[]) => void;
  setCurrentSpec: (spec: SpecWithVersion | null) => void;
  setLoading: (loading: boolean) => void;
  setExploring: (exploring: boolean) => void;
  setPlanning: (planning: boolean) => void;
  setError: (error: string | null) => void;
}

export const useSpecStore = create<SpecState>((set, get) => ({
  specs: [],
  currentSpec: null,
  currentVersions: [],
  selectedVersion: null,
  selectedVersionId: null,
  activeTab: 'chat',
  specTickets: [],
  liveLogs: [],
  currentEta: null,
  conversationMessages: [],
  isAgentThinking: false,
  brainstormLogs: [],
  isGeneratingSpec: false,
  generatingVersionNumber: null,
  scrollToProgress: false,
  isLoading: false,
  isExploring: false,
  isPlanning: false,
  error: null,

  loadSpecs: async (boardId: string) => {
    set({ isLoading: true, error: null });
    try {
      const specs = await invoke<SpecWithVersion[]>('get_specs_with_versions', { boardId });
      const { currentSpec } = get();
      
      // Check if currentSpec is still in the loaded list
      // If not (e.g., was deleted), clear it
      const currentStillExists = currentSpec && 
        specs.some(s => s.id === currentSpec.id);
      
      if (currentSpec && !currentStillExists) {
        set({ specs, currentSpec: null, currentVersions: [], selectedVersion: null, isLoading: false });
      } else if (currentSpec && currentStillExists) {
        // Update currentSpec with fresh data from the list
        const updated = specs.find(s => s.id === currentSpec.id);
        set({ 
          specs, 
          currentSpec: updated || null,
          selectedVersion: updated?.latestVersion || null,
          isLoading: false 
        });
      } else {
        set({ specs, isLoading: false });
      }
    } catch (error) {
      logger.error('Failed to load specs', error);
      set({ error: String(error), isLoading: false });
    }
  },

  loadAllSpecs: async () => {
    set({ isLoading: true, error: null });
    try {
      const specs = await invoke<SpecWithVersion[]>('get_all_specs_with_versions');
      const { currentSpec } = get();
      
      // Check if currentSpec is still in the loaded list
      // If not (e.g., was deleted), clear it
      const currentStillExists = currentSpec && 
        specs.some(s => s.id === currentSpec.id);
      
      if (currentSpec && !currentStillExists) {
        set({ specs, currentSpec: null, currentVersions: [], selectedVersion: null, isLoading: false });
      } else if (currentSpec && currentStillExists) {
        // Update currentSpec with fresh data from the list
        const updated = specs.find(s => s.id === currentSpec.id);
        set({ 
          specs, 
          currentSpec: updated || null, 
          selectedVersion: updated?.latestVersion || null,
          isLoading: false 
        });
      } else {
        set({ specs, isLoading: false });
      }
    } catch (error) {
      logger.error('Failed to load all specs', error);
      set({ error: String(error), isLoading: false });
    }
  },

  getSpec: async (id: string) => {
    try {
      const spec = await invoke<SpecWithVersion>('get_spec_with_version', { id });
      return spec;
    } catch (error) {
      logger.error('Failed to get spec', error);
      throw error;
    }
  },

  createSpec: async (input: CreateSpecInput) => {
    set({ isLoading: true, error: null });
    try {
      const baseSpec = await invoke<Spec>('create_spec', {
        input: {
          boardId: input.boardId,
          targetBoardId: input.targetBoardId,
          projectId: input.projectId,
          name: input.name,
          userInput: input.userInput,
          model: input.model,
        },
      });
      
      // Fetch the full spec with version data
      const spec = await invoke<SpecWithVersion>('get_spec_with_version', { id: baseSpec.id });
      
      const { specs } = get();
      set({ 
        specs: [spec, ...specs],
        currentSpec: spec,
        currentVersions: spec.latestVersion ? [spec.latestVersion] : [],
        selectedVersion: spec.latestVersion || null,
        isLoading: false 
      });
      
      logger.info('Created spec', { id: spec.id, name: spec.name });
      return baseSpec;
    } catch (error) {
      logger.error('Failed to create spec', error);
      set({ error: String(error), isLoading: false });
      throw error;
    }
  },

  updateSpec: async (id: string, updates: UpdateSpecInput) => {
    try {
      const baseSpec = await invoke<Spec>('update_spec', {
        id,
        name: updates.name,
        userInput: updates.userInput,
        model: updates.model,
      });
      
      // Fetch the updated spec with version data
      const spec = await invoke<SpecWithVersion>('get_spec_with_version', { id });
      
      const { specs, currentSpec } = get();
      set({
        specs: specs.map(s => s.id === id ? spec : s),
        currentSpec: currentSpec?.id === id ? spec : currentSpec,
        selectedVersion: currentSpec?.id === id ? spec.latestVersion || null : get().selectedVersion,
      });
      
      return baseSpec;
    } catch (error) {
      logger.error('Failed to update spec', error);
      throw error;
    }
  },

  deleteSpec: async (id: string, deleteTickets = false) => {
    try {
      if (deleteTickets) {
        const count = await invoke<number>('delete_spec_with_tickets', { id });
        logger.info('Deleted spec with tickets', { id, ticketsDeleted: count });
      } else {
        await invoke('delete_spec', { id });
        logger.info('Deleted spec', { id });
      }
      
      const { specs, currentSpec } = get();
      set({
        specs: specs.filter(s => s.id !== id),
        currentSpec: currentSpec?.id === id ? null : currentSpec,
      });
    } catch (error) {
      logger.error('Failed to delete spec', error);
      throw error;
    }
  },

  selectSpec: (spec: SpecWithVersion | null) => {
    set({ 
      currentSpec: spec,
      currentVersions: spec?.latestVersion ? [spec.latestVersion] : [],
      selectedVersion: spec?.latestVersion || null,
      selectedVersionId: spec?.latestVersion?.id ?? null,
      activeTab: 'chat',
    });
  },

  selectSpecForProgress: (spec: SpecWithVersion) => {
    set({
      currentSpec: spec,
      currentVersions: spec.latestVersion ? [spec.latestVersion] : [],
      selectedVersion: spec.latestVersion || null,
      selectedVersionId: spec.latestVersion?.id ?? null,
      activeTab: 'versions',
      scrollToProgress: true,
    });
  },

  setScrollToProgress: (scroll: boolean) => set({ scrollToProgress: scroll }),

  // Version management
  loadVersions: async (specId: string) => {
    try {
      const versions = await invoke<SpecVersion[]>('get_spec_versions', { specId });
      set({ currentVersions: versions });
    } catch (error) {
      logger.error('Failed to load spec versions', error);
      throw error;
    }
  },

  selectVersion: (version: SpecVersion | null) => {
    set({ selectedVersion: version, selectedVersionId: version?.id ?? null });
  },

  selectVersionById: (versionId: string) => {
    const { currentVersions } = get();
    const version = currentVersions.find(v => v.id === versionId) || null;
    set({ selectedVersion: version, selectedVersionId: versionId, activeTab: 'versions' });
  },

  setActiveTab: (tab: 'chat' | 'versions') => {
    set({ activeTab: tab });
  },

  createNewVersion: async (specId: string) => {
    try {
      const version = await invoke<SpecVersion>('create_new_spec_version', { specId });
      
      // Refresh spec and versions
      const spec = await get().getSpec(specId);
      const versions = await invoke<SpecVersion[]>('get_spec_versions', { specId });
      
      const { specs } = get();
      set({
        specs: specs.map(s => s.id === specId ? spec : s),
        currentSpec: spec,
        currentVersions: versions,
        selectedVersion: version,
      });
      
      logger.info('Created new spec version', { specId, versionNumber: version.versionNumber });
      return version;
    } catch (error) {
      logger.error('Failed to create new spec version', error);
      throw error;
    }
  },

  setStatus: async (id: string, status: string) => {
    try {
      await invoke('set_spec_status', { id, status });
      
      // Refresh the spec
      const spec = await get().getSpec(id);
      const { specs, currentSpec } = get();
      set({
        specs: specs.map(s => s.id === id ? spec : s),
        currentSpec: currentSpec?.id === id ? spec : currentSpec,
      });
    } catch (error) {
      logger.error('Failed to set spec status', error);
      throw error;
    }
  },

  appendExploration: async (id: string, query: string, response: string) => {
    try {
      await invoke('append_exploration', { id, query, response });
      
      // Refresh the spec
      const spec = await get().getSpec(id);
      const { specs, currentSpec } = get();
      set({
        specs: specs.map(s => s.id === id ? spec : s),
        currentSpec: currentSpec?.id === id ? spec : currentSpec,
      });
    } catch (error) {
      logger.error('Failed to append exploration', error);
      throw error;
    }
  },

  setPlan: async (id: string, markdown: string, json?: unknown) => {
    try {
      await invoke('set_spec_plan', { id, markdown, json });
      
      // Refresh the spec
      const spec = await get().getSpec(id);
      const { specs, currentSpec } = get();
      set({
        specs: specs.map(s => s.id === id ? spec : s),
        currentSpec: currentSpec?.id === id ? spec : currentSpec,
      });
    } catch (error) {
      logger.error('Failed to set plan', error);
      throw error;
    }
  },

  approvePlan: async (id: string) => {
    try {
      await invoke('approve_plan', { id });
      
      // Refresh the spec
      const spec = await get().getSpec(id);
      const { specs, currentSpec } = get();
      set({
        specs: specs.map(s => s.id === id ? spec : s),
        currentSpec: currentSpec?.id === id ? spec : currentSpec,
      });
      
      logger.info('Approved plan', { id });
    } catch (error) {
      logger.error('Failed to approve plan', error);
      throw error;
    }
  },

  loadSpecTickets: async (id: string) => {
    try {
      const tickets = await invoke<Ticket[]>('get_spec_tickets', { id });
      set({ specTickets: tickets });
    } catch (error) {
      logger.error('Failed to load spec tickets', error);
      throw error;
    }
  },

  // Pause/Resume/Halt controls
  pauseWork: async (id: string) => {
    try {
      await invoke('pause_spec_work', { specId: id });
      
      // Refresh the spec
      const spec = await get().getSpec(id);
      const { specs, currentSpec } = get();
      set({
        specs: specs.map(s => s.id === id ? spec : s),
        currentSpec: currentSpec?.id === id ? spec : currentSpec,
      });
      
      logger.info('Paused work on spec', { id });
    } catch (error) {
      logger.error('Failed to pause spec work', error);
      throw error;
    }
  },

  resumeWork: async (id: string) => {
    try {
      await invoke('resume_spec_work', { specId: id });
      
      // Refresh the spec
      const spec = await get().getSpec(id);
      const { specs, currentSpec } = get();
      set({
        specs: specs.map(s => s.id === id ? spec : s),
        currentSpec: currentSpec?.id === id ? spec : currentSpec,
      });
      
      logger.info('Resumed work on spec', { id });
    } catch (error) {
      logger.error('Failed to resume spec work', error);
      throw error;
    }
  },

  haltWork: async (id: string) => {
    try {
      await invoke('halt_spec_work', { specId: id });
      
      // Refresh the spec
      const spec = await get().getSpec(id);
      const { specs, currentSpec } = get();
      set({
        specs: specs.map(s => s.id === id ? spec : s),
        currentSpec: currentSpec?.id === id ? spec : currentSpec,
      });
      
      logger.info('Halted work on spec', { id });
    } catch (error) {
      logger.error('Failed to halt spec work', error);
      throw error;
    }
  },

  // ETA
  loadEta: async (id: string) => {
    try {
      const eta = await invoke<SpecEta>('get_spec_eta', { specId: id });
      set({ currentEta: eta });
    } catch (error) {
      logger.error('Failed to load ETA', error);
      // Don't throw - ETA is non-critical
      set({ currentEta: null });
    }
  },

  // Live log management
  addLogEntry: (entry) => {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
    const newEntry: SpecLogEntry = { ...entry, id };
    
    set((state) => ({
      // Keep last 500 entries to avoid memory issues
      liveLogs: [...state.liveLogs.slice(-499), newEntry],
    }));
  },
  
  clearLogs: (specId) => {
    if (specId) {
      set((state) => ({
        liveLogs: state.liveLogs.filter((log) => log.specId !== specId),
      }));
    } else {
      set({ liveLogs: [] });
    }
  },

  // Conversation management
  setConversationMessages: (messages) => set({ conversationMessages: messages }),
  
  addConversationMessage: (message) => {
    set((state) => {
      // Don't add duplicates
      if (state.conversationMessages.some(m => m.id === message.id)) {
        return state;
      }
      // Only stop thinking when an assistant message arrives (not user messages)
      const shouldStopThinking = message.role === 'assistant';
      return {
        conversationMessages: [...state.conversationMessages, message],
        // Clear thinking state when assistant responds, but DON'T clear logs
        // Logs should persist so user can see what the agent was thinking
        isAgentThinking: shouldStopThinking ? false : state.isAgentThinking,
      };
    });
  },
  
  setAgentThinking: (thinking) => set({ isAgentThinking: thinking }),
  
  clearConversation: () => set({ 
    conversationMessages: [], 
    isAgentThinking: false,
    brainstormLogs: [],
    isGeneratingSpec: false,
    generatingVersionNumber: null,
  }),
  
  // Brainstorm log management
  addBrainstormLog: (message) => {
    set((state) => ({
      brainstormLogs: [...state.brainstormLogs.slice(-3), message], // Keep last 4 for rolling visual effect
    }));
  },
  
  clearBrainstormLogs: () => set({ brainstormLogs: [] }),
  
  setGeneratingSpec: (generating, versionNumber) => set({ 
    isGeneratingSpec: generating,
    generatingVersionNumber: versionNumber ?? null,
    isAgentThinking: generating, // Also set thinking state
  }),

  // State setters
  setSpecs: (specs) => set({ specs }),
  setCurrentSpec: (spec) => {
    const state = get();
    const updates: Partial<SpecState> = { currentSpec: spec };
    
    // Keep selectedVersion and currentVersions in sync when the
    // latest version data changes (e.g. after plan_generated or spec_updated SSE events).
    if (spec?.latestVersion && state.selectedVersion) {
      if (spec.latestVersion.id === state.selectedVersion.id) {
        updates.selectedVersion = spec.latestVersion;
        updates.currentVersions = state.currentVersions.map(v =>
          v.id === spec.latestVersion!.id ? spec.latestVersion! : v
        );
      }
    }
    
    set(updates);
  },
  setLoading: (loading) => set({ isLoading: loading }),
  setExploring: (exploring) => set({ isExploring: exploring }),
  setPlanning: (planning) => set({ isPlanning: planning }),
  setError: (error) => set({ error }),
}));
