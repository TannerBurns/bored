import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { Spec, CreateSpecInput, UpdateSpecInput, Ticket, SpecEta } from '../types';
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
  specs: Spec[];
  currentSpec: Spec | null;
  specTickets: Ticket[];
  /** Real-time log entries from agent output */
  liveLogs: SpecLogEntry[];
  /** ETA information for the current spec */
  currentEta: SpecEta | null;
  isLoading: boolean;
  isExploring: boolean;
  isPlanning: boolean;
  error: string | null;

  // Actions
  loadSpecs: (boardId: string) => Promise<void>;
  loadAllSpecs: () => Promise<void>;
  getSpec: (id: string) => Promise<Spec>;
  createSpec: (input: CreateSpecInput) => Promise<Spec>;
  updateSpec: (id: string, updates: UpdateSpecInput) => Promise<Spec>;
  deleteSpec: (id: string, deleteTickets?: boolean) => Promise<void>;
  selectSpec: (spec: Spec | null) => void;
  
  // Status management
  setStatus: (id: string, status: string) => Promise<void>;
  
  // Exploration
  appendExploration: (id: string, query: string, response: string) => Promise<void>;
  
  // Plan management
  setPlan: (id: string, markdown: string, json?: unknown) => Promise<void>;
  approvePlan: (id: string) => Promise<void>;
  
  // Get tickets created from spec
  loadSpecTickets: (id: string) => Promise<void>;
  
  // Pause/Resume/Halt controls
  pauseWork: (id: string) => Promise<void>;
  resumeWork: (id: string) => Promise<void>;
  haltWork: (id: string) => Promise<void>;
  
  // ETA
  loadEta: (id: string) => Promise<void>;
  
  // Live log management
  addLogEntry: (entry: Omit<SpecLogEntry, 'id'>) => void;
  clearLogs: (specId?: string) => void;
  
  // State setters
  setSpecs: (specs: Spec[]) => void;
  setCurrentSpec: (spec: Spec | null) => void;
  setLoading: (loading: boolean) => void;
  setExploring: (exploring: boolean) => void;
  setPlanning: (planning: boolean) => void;
  setError: (error: string | null) => void;
}

export const useSpecStore = create<SpecState>((set, get) => ({
  specs: [],
  currentSpec: null,
  specTickets: [],
  liveLogs: [],
  currentEta: null,
  isLoading: false,
  isExploring: false,
  isPlanning: false,
  error: null,

  loadSpecs: async (boardId: string) => {
    set({ isLoading: true, error: null });
    try {
      const specs = await invoke<Spec[]>('get_specs', { boardId });
      const { currentSpec } = get();
      
      // Check if currentSpec is still in the loaded list
      // If not (e.g., was deleted), clear it
      const currentStillExists = currentSpec && 
        specs.some(s => s.id === currentSpec.id);
      
      if (currentSpec && !currentStillExists) {
        set({ specs, currentSpec: null, isLoading: false });
      } else if (currentSpec && currentStillExists) {
        // Update currentSpec with fresh data from the list
        const updated = specs.find(s => s.id === currentSpec.id);
        set({ specs, currentSpec: updated || null, isLoading: false });
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
      const specs = await invoke<Spec[]>('get_all_specs');
      const { currentSpec } = get();
      
      // Check if currentSpec is still in the loaded list
      // If not (e.g., was deleted), clear it
      const currentStillExists = currentSpec && 
        specs.some(s => s.id === currentSpec.id);
      
      if (currentSpec && !currentStillExists) {
        set({ specs, currentSpec: null, isLoading: false });
      } else if (currentSpec && currentStillExists) {
        // Update currentSpec with fresh data from the list
        const updated = specs.find(s => s.id === currentSpec.id);
        set({ specs, currentSpec: updated || null, isLoading: false });
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
      const spec = await invoke<Spec>('get_spec', { id });
      return spec;
    } catch (error) {
      logger.error('Failed to get spec', error);
      throw error;
    }
  },

  createSpec: async (input: CreateSpecInput) => {
    set({ isLoading: true, error: null });
    try {
      const spec = await invoke<Spec>('create_spec', {
        input: {
          boardId: input.boardId,
          targetBoardId: input.targetBoardId,
          projectId: input.projectId,
          name: input.name,
          userInput: input.userInput,
          agentPref: input.agentPref,
          model: input.model,
        },
      });
      
      const { specs } = get();
      set({ 
        specs: [spec, ...specs],
        currentSpec: spec,
        isLoading: false 
      });
      
      logger.info('Created spec', { id: spec.id, name: spec.name });
      return spec;
    } catch (error) {
      logger.error('Failed to create spec', error);
      set({ error: String(error), isLoading: false });
      throw error;
    }
  },

  updateSpec: async (id: string, updates: UpdateSpecInput) => {
    try {
      const spec = await invoke<Spec>('update_spec', {
        id,
        name: updates.name,
        userInput: updates.userInput,
        agentPref: updates.agentPref,
        model: updates.model,
      });
      
      const { specs, currentSpec } = get();
      set({
        specs: specs.map(s => s.id === id ? spec : s),
        currentSpec: currentSpec?.id === id ? spec : currentSpec,
      });
      
      return spec;
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

  selectSpec: (spec: Spec | null) => {
    set({ currentSpec: spec });
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

  // State setters
  setSpecs: (specs) => set({ specs }),
  setCurrentSpec: (spec) => set({ currentSpec: spec }),
  setLoading: (loading) => set({ isLoading: loading }),
  setExploring: (exploring) => set({ isExploring: exploring }),
  setPlanning: (planning) => set({ isPlanning: planning }),
  setError: (error) => set({ error }),
}));
