import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/tauri';
import type { Scratchpad, CreateScratchpadInput, UpdateScratchpadInput, Ticket } from '../types';
import { logger } from '../lib/logger';

/** A single log entry from the planner agent */
export interface PlannerLogEntry {
  id: string;
  scratchpadId: string;
  phase: 'exploration' | 'planning';
  level: 'info' | 'output' | 'error';
  message: string;
  timestamp: string;
}

interface PlannerState {
  scratchpads: Scratchpad[];
  currentScratchpad: Scratchpad | null;
  scratchpadTickets: Ticket[];
  /** Real-time log entries from agent output */
  liveLogs: PlannerLogEntry[];
  isLoading: boolean;
  isExploring: boolean;
  isPlanning: boolean;
  error: string | null;

  // Actions
  loadScratchpads: (boardId: string) => Promise<void>;
  loadAllScratchpads: () => Promise<void>;
  getScratchpad: (id: string) => Promise<Scratchpad>;
  createScratchpad: (input: CreateScratchpadInput) => Promise<Scratchpad>;
  updateScratchpad: (id: string, updates: UpdateScratchpadInput) => Promise<Scratchpad>;
  deleteScratchpad: (id: string, deleteTickets?: boolean) => Promise<void>;
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
  
  // Live log management
  addLogEntry: (entry: Omit<PlannerLogEntry, 'id'>) => void;
  clearLogs: (scratchpadId?: string) => void;
  
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
  liveLogs: [],
  isLoading: false,
  isExploring: false,
  isPlanning: false,
  error: null,

  loadScratchpads: async (boardId: string) => {
    set({ isLoading: true, error: null });
    try {
      const scratchpads = await invoke<Scratchpad[]>('get_scratchpads', { boardId });
      const { currentScratchpad } = get();
      
      // Check if currentScratchpad is still in the loaded list
      // If not (e.g., was deleted), clear it
      const currentStillExists = currentScratchpad && 
        scratchpads.some(s => s.id === currentScratchpad.id);
      
      if (currentScratchpad && !currentStillExists) {
        set({ scratchpads, currentScratchpad: null, isLoading: false });
      } else if (currentScratchpad && currentStillExists) {
        // Update currentScratchpad with fresh data from the list
        const updated = scratchpads.find(s => s.id === currentScratchpad.id);
        set({ scratchpads, currentScratchpad: updated || null, isLoading: false });
      } else {
        set({ scratchpads, isLoading: false });
      }
    } catch (error) {
      logger.error('Failed to load scratchpads', error);
      set({ error: String(error), isLoading: false });
    }
  },

  loadAllScratchpads: async () => {
    set({ isLoading: true, error: null });
    try {
      const scratchpads = await invoke<Scratchpad[]>('get_all_scratchpads');
      const { currentScratchpad } = get();
      
      // Check if currentScratchpad is still in the loaded list
      // If not (e.g., was deleted), clear it
      const currentStillExists = currentScratchpad && 
        scratchpads.some(s => s.id === currentScratchpad.id);
      
      if (currentScratchpad && !currentStillExists) {
        set({ scratchpads, currentScratchpad: null, isLoading: false });
      } else if (currentScratchpad && currentStillExists) {
        // Update currentScratchpad with fresh data from the list
        const updated = scratchpads.find(s => s.id === currentScratchpad.id);
        set({ scratchpads, currentScratchpad: updated || null, isLoading: false });
      } else {
        set({ scratchpads, isLoading: false });
      }
    } catch (error) {
      logger.error('Failed to load all scratchpads', error);
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

  deleteScratchpad: async (id: string, deleteTickets = false) => {
    try {
      if (deleteTickets) {
        const count = await invoke<number>('delete_scratchpad_with_tickets', { id });
        logger.info('Deleted scratchpad with tickets', { id, ticketsDeleted: count });
      } else {
        await invoke('delete_scratchpad', { id });
        logger.info('Deleted scratchpad', { id });
      }
      
      const { scratchpads, currentScratchpad } = get();
      set({
        scratchpads: scratchpads.filter(s => s.id !== id),
        currentScratchpad: currentScratchpad?.id === id ? null : currentScratchpad,
      });
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

  // Live log management
  addLogEntry: (entry) => {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
    const newEntry: PlannerLogEntry = { ...entry, id };
    
    set((state) => ({
      // Keep last 500 entries to avoid memory issues
      liveLogs: [...state.liveLogs.slice(-499), newEntry],
    }));
  },
  
  clearLogs: (scratchpadId) => {
    if (scratchpadId) {
      set((state) => ({
        liveLogs: state.liveLogs.filter((log) => log.scratchpadId !== scratchpadId),
      }));
    } else {
      set({ liveLogs: [] });
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
