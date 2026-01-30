import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/tauri';
import type { Scratchpad, CreateScratchpadInput, UpdateScratchpadInput, Ticket } from '../types';
import { logger } from '../lib/logger';

interface PlannerState {
  scratchpads: Scratchpad[];
  currentScratchpad: Scratchpad | null;
  scratchpadTickets: Ticket[];
  isLoading: boolean;
  isExploring: boolean;
  isPlanning: boolean;
  error: string | null;

  // Actions
  loadScratchpads: (boardId: string) => Promise<void>;
  getScratchpad: (id: string) => Promise<Scratchpad>;
  createScratchpad: (input: CreateScratchpadInput) => Promise<Scratchpad>;
  updateScratchpad: (id: string, updates: UpdateScratchpadInput) => Promise<Scratchpad>;
  deleteScratchpad: (id: string) => Promise<void>;
  selectScratchpad: (scratchpad: Scratchpad | null) => void;
  
  // Status management
  setStatus: (id: string, status: string) => Promise<void>;
  
  // Exploration
  appendExploration: (id: string, query: string, response: string) => Promise<void>;
  
  // Plan management
  setPlan: (id: string, markdown: string, json?: unknown) => Promise<void>;
  approvePlan: (id: string) => Promise<void>;
  
  // Get tickets created from scratchpad
  loadScratchpadTickets: (id: string) => Promise<void>;
  
  // State setters
  setScratchpads: (scratchpads: Scratchpad[]) => void;
  setCurrentScratchpad: (scratchpad: Scratchpad | null) => void;
  setLoading: (loading: boolean) => void;
  setExploring: (exploring: boolean) => void;
  setPlanning: (planning: boolean) => void;
  setError: (error: string | null) => void;
}

export const usePlannerStore = create<PlannerState>((set, get) => ({
  scratchpads: [],
  currentScratchpad: null,
  scratchpadTickets: [],
  isLoading: false,
  isExploring: false,
  isPlanning: false,
  error: null,

  loadScratchpads: async (boardId: string) => {
    set({ isLoading: true, error: null });
    try {
      const scratchpads = await invoke<Scratchpad[]>('get_scratchpads', { boardId });
      set({ scratchpads, isLoading: false });
    } catch (error) {
      logger.error('Failed to load scratchpads', error);
      set({ error: String(error), isLoading: false });
    }
  },

  getScratchpad: async (id: string) => {
    try {
      const scratchpad = await invoke<Scratchpad>('get_scratchpad', { id });
      return scratchpad;
    } catch (error) {
      logger.error('Failed to get scratchpad', error);
      throw error;
    }
  },

  createScratchpad: async (input: CreateScratchpadInput) => {
    set({ isLoading: true, error: null });
    try {
      const scratchpad = await invoke<Scratchpad>('create_scratchpad', {
        boardId: input.boardId,
        projectId: input.projectId,
        name: input.name,
        userInput: input.userInput,
        agentPref: input.agentPref,
        model: input.model,
      });
      
      const { scratchpads } = get();
      set({ 
        scratchpads: [scratchpad, ...scratchpads],
        currentScratchpad: scratchpad,
        isLoading: false 
      });
      
      logger.info('Created scratchpad', { id: scratchpad.id, name: scratchpad.name });
      return scratchpad;
    } catch (error) {
      logger.error('Failed to create scratchpad', error);
      set({ error: String(error), isLoading: false });
      throw error;
    }
  },

  updateScratchpad: async (id: string, updates: UpdateScratchpadInput) => {
    try {
      const scratchpad = await invoke<Scratchpad>('update_scratchpad', {
        id,
        name: updates.name,
        userInput: updates.userInput,
        agentPref: updates.agentPref,
        model: updates.model,
      });
      
      const { scratchpads, currentScratchpad } = get();
      set({
        scratchpads: scratchpads.map(s => s.id === id ? scratchpad : s),
        currentScratchpad: currentScratchpad?.id === id ? scratchpad : currentScratchpad,
      });
      
      return scratchpad;
    } catch (error) {
      logger.error('Failed to update scratchpad', error);
      throw error;
    }
  },

  deleteScratchpad: async (id: string) => {
    try {
      await invoke('delete_scratchpad', { id });
      
      const { scratchpads, currentScratchpad } = get();
      set({
        scratchpads: scratchpads.filter(s => s.id !== id),
        currentScratchpad: currentScratchpad?.id === id ? null : currentScratchpad,
      });
      
      logger.info('Deleted scratchpad', { id });
    } catch (error) {
      logger.error('Failed to delete scratchpad', error);
      throw error;
    }
  },

  selectScratchpad: (scratchpad: Scratchpad | null) => {
    set({ currentScratchpad: scratchpad });
  },

  setStatus: async (id: string, status: string) => {
    try {
      await invoke('set_scratchpad_status', { id, status });
      
      // Refresh the scratchpad
      const scratchpad = await get().getScratchpad(id);
      const { scratchpads, currentScratchpad } = get();
      set({
        scratchpads: scratchpads.map(s => s.id === id ? scratchpad : s),
        currentScratchpad: currentScratchpad?.id === id ? scratchpad : currentScratchpad,
      });
    } catch (error) {
      logger.error('Failed to set scratchpad status', error);
      throw error;
    }
  },

  appendExploration: async (id: string, query: string, response: string) => {
    try {
      await invoke('append_exploration', { id, query, response });
      
      // Refresh the scratchpad
      const scratchpad = await get().getScratchpad(id);
      const { scratchpads, currentScratchpad } = get();
      set({
        scratchpads: scratchpads.map(s => s.id === id ? scratchpad : s),
        currentScratchpad: currentScratchpad?.id === id ? scratchpad : currentScratchpad,
      });
    } catch (error) {
      logger.error('Failed to append exploration', error);
      throw error;
    }
  },

  setPlan: async (id: string, markdown: string, json?: unknown) => {
    try {
      await invoke('set_scratchpad_plan', { id, markdown, json });
      
      // Refresh the scratchpad
      const scratchpad = await get().getScratchpad(id);
      const { scratchpads, currentScratchpad } = get();
      set({
        scratchpads: scratchpads.map(s => s.id === id ? scratchpad : s),
        currentScratchpad: currentScratchpad?.id === id ? scratchpad : currentScratchpad,
      });
    } catch (error) {
      logger.error('Failed to set plan', error);
      throw error;
    }
  },

  approvePlan: async (id: string) => {
    try {
      await invoke('approve_plan', { id });
      
      // Refresh the scratchpad
      const scratchpad = await get().getScratchpad(id);
      const { scratchpads, currentScratchpad } = get();
      set({
        scratchpads: scratchpads.map(s => s.id === id ? scratchpad : s),
        currentScratchpad: currentScratchpad?.id === id ? scratchpad : currentScratchpad,
      });
      
      logger.info('Approved plan', { id });
    } catch (error) {
      logger.error('Failed to approve plan', error);
      throw error;
    }
  },

  loadScratchpadTickets: async (id: string) => {
    try {
      const tickets = await invoke<Ticket[]>('get_scratchpad_tickets', { id });
      set({ scratchpadTickets: tickets });
    } catch (error) {
      logger.error('Failed to load scratchpad tickets', error);
      throw error;
    }
  },

  // State setters
  setScratchpads: (scratchpads) => set({ scratchpads }),
  setCurrentScratchpad: (scratchpad) => set({ currentScratchpad: scratchpad }),
  setLoading: (loading) => set({ isLoading: loading }),
  setExploring: (exploring) => set({ isExploring: exploring }),
  setPlanning: (planning) => set({ isPlanning: planning }),
  setError: (error) => set({ error }),
}));
