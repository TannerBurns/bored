import { create } from 'zustand';
import type { ValidationSession, ValidationMessage } from '../types';
import {
  createValidationSession as apiCreateSession,
  getValidationSession as apiGetSession,
  getValidationSessions as apiGetSessions,
  getValidationMessages as apiGetMessages,
  sendValidationMessage as apiSendMessage,
  updateValidationSessionStatus as apiUpdateStatus,
  deleteValidationSession as apiDeleteSession,
  createFixTasks as apiCreateFixTasks,
  pushBranch as apiPushBranch,
  createPullRequest as apiCreatePR,
  getBranchDiff as apiGetDiff,
  getBranchDiffFiles as apiGetDiffFiles,
} from '../lib/tauri';
import type { FixTask, PushResult, PullRequestResult, BranchDiff, FileDiff } from '../types';
import { logger } from '../lib/logger';

/** A log entry from the app runner */
export interface AppLogEntry {
  id: string;
  sessionId: string;
  stream: 'stdout' | 'stderr';
  message: string;
  timestamp: string;
}

interface ValidationState {
  // Session state
  sessions: ValidationSession[];
  currentSession: ValidationSession | null;

  // Chat state
  messages: ValidationMessage[];
  isAgentThinking: boolean;

  // App runner state
  appLogs: AppLogEntry[];
  isAppRunning: boolean;

  // Loading / error
  isLoading: boolean;
  error: string | null;

  // Session actions
  loadSessions: (ticketId: string) => Promise<void>;
  createSession: (input: {
    ticketId: string;
    projectId?: string;
    appCommand?: string;
    appPort?: number;
    agentType?: 'cursor' | 'claude';
  }) => Promise<ValidationSession>;
  selectSession: (session: ValidationSession | null) => void;
  updateSessionStatus: (sessionId: string, status: string) => Promise<void>;
  deleteSession: (sessionId: string) => Promise<void>;
  refreshSession: (sessionId: string) => Promise<void>;

  // Chat actions
  loadMessages: (sessionId: string) => Promise<void>;
  sendMessage: (sessionId: string, content: string) => Promise<ValidationMessage>;
  addMessage: (message: ValidationMessage) => void;
  setAgentThinking: (thinking: boolean) => void;
  clearMessages: () => void;

  // App log actions
  addAppLog: (log: AppLogEntry) => void;
  clearAppLogs: () => void;
  setAppRunning: (running: boolean) => void;

  // Next steps actions
  pushBranch: (ticketId: string) => Promise<PushResult>;
  createPullRequest: (ticketId: string, title?: string, body?: string) => Promise<PullRequestResult>;
  getBranchDiff: (ticketId: string) => Promise<BranchDiff>;
  getBranchDiffFiles: (ticketId: string) => Promise<FileDiff[]>;

  // Fix task actions
  createFixTasks: (sessionId: string, ticketId: string, tasks: FixTask[]) => Promise<string[]>;

  // Reset
  reset: () => void;
}

export const useValidationStore = create<ValidationState>((set) => ({
  // Initial state
  sessions: [],
  currentSession: null,
  messages: [],
  isAgentThinking: false,
  appLogs: [],
  isAppRunning: false,
  isLoading: false,
  error: null,

  // Session actions
  loadSessions: async (ticketId: string) => {
    try {
      const sessions = await apiGetSessions(ticketId);
      set({ sessions });
    } catch (e) {
      logger.error('Failed to load validation sessions', e);
      set({ error: String(e) });
    }
  },

  createSession: async (input) => {
    try {
      set({ isLoading: true, error: null });
      const session = await apiCreateSession(input);
      set((state) => ({
        sessions: [session, ...state.sessions],
        currentSession: session,
        isLoading: false,
      }));
      return session;
    } catch (e) {
      logger.error('Failed to create validation session', e);
      set({ isLoading: false, error: String(e) });
      throw e;
    }
  },

  selectSession: (session) => {
    set({
      currentSession: session,
      messages: [],
      appLogs: [],
      isAgentThinking: false,
      error: null,
    });
  },

  updateSessionStatus: async (sessionId, status) => {
    try {
      await apiUpdateStatus(sessionId, status);
      const session = await apiGetSession(sessionId);
      set((state) => ({
        currentSession:
          state.currentSession?.id === sessionId ? session : state.currentSession,
        sessions: state.sessions.map((s) =>
          s.id === sessionId ? session : s
        ),
      }));
    } catch (e) {
      logger.error('Failed to update validation session status', e);
      set({ error: String(e) });
    }
  },

  deleteSession: async (sessionId) => {
    try {
      await apiDeleteSession(sessionId);
      set((state) => ({
        sessions: state.sessions.filter((s) => s.id !== sessionId),
        currentSession:
          state.currentSession?.id === sessionId ? null : state.currentSession,
      }));
    } catch (e) {
      logger.error('Failed to delete validation session', e);
      set({ error: String(e) });
    }
  },

  refreshSession: async (sessionId) => {
    try {
      const session = await apiGetSession(sessionId);
      set((state) => ({
        currentSession:
          state.currentSession?.id === sessionId ? session : state.currentSession,
        sessions: state.sessions.map((s) =>
          s.id === sessionId ? session : s
        ),
      }));
    } catch (e) {
      logger.error('Failed to refresh validation session', e);
    }
  },

  // Chat actions
  loadMessages: async (sessionId) => {
    try {
      const messages = await apiGetMessages(sessionId);
      set({ messages });
    } catch (e) {
      logger.error('Failed to load validation messages', e);
      set({ error: String(e) });
    }
  },

  sendMessage: async (sessionId, content) => {
    try {
      set({ isAgentThinking: true, appLogs: [] });
      const message = await apiSendMessage(sessionId, content);
      set((state) => ({
        messages: [...state.messages, message],
      }));
      return message;
    } catch (e) {
      logger.error('Failed to send validation message', e);
      set({ isAgentThinking: false, error: String(e) });
      throw e;
    }
  },

  addMessage: (message) => {
    set((state) => {
      // Deduplicate
      if (state.messages.some((m) => m.id === message.id)) {
        return state;
      }
      return {
        messages: [...state.messages, message],
        // Clear thinking state when assistant responds
        isAgentThinking: message.role === 'assistant' ? false : state.isAgentThinking,
      };
    });
  },

  setAgentThinking: (thinking) => set({ isAgentThinking: thinking }),

  clearMessages: () => set({ messages: [] }),

  // App log actions
  addAppLog: (log) => {
    set((state) => ({
      appLogs: [...state.appLogs, log],
    }));
  },

  clearAppLogs: () => set({ appLogs: [] }),

  setAppRunning: (running) => set({ isAppRunning: running }),

  // Next steps actions
  pushBranch: async (ticketId) => {
    try {
      set({ isLoading: true, error: null });
      const result = await apiPushBranch(ticketId);
      set({ isLoading: false });
      return result;
    } catch (e) {
      set({ isLoading: false, error: String(e) });
      throw e;
    }
  },

  createPullRequest: async (ticketId, title, body) => {
    try {
      set({ isLoading: true, error: null });
      const result = await apiCreatePR(ticketId, title, body);
      set({ isLoading: false });
      return result;
    } catch (e) {
      set({ isLoading: false, error: String(e) });
      throw e;
    }
  },

  getBranchDiff: async (ticketId) => {
    try {
      return await apiGetDiff(ticketId);
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  getBranchDiffFiles: async (ticketId) => {
    try {
      return await apiGetDiffFiles(ticketId);
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  // Fix task actions
  createFixTasks: async (sessionId, ticketId, tasks) => {
    try {
      set({ isLoading: true, error: null });
      const taskIds = await apiCreateFixTasks({ sessionId, ticketId, tasks });
      set({ isLoading: false });
      return taskIds;
    } catch (e) {
      set({ isLoading: false, error: String(e) });
      throw e;
    }
  },

  // Reset
  reset: () =>
    set({
      sessions: [],
      currentSession: null,
      messages: [],
      isAgentThinking: false,
      appLogs: [],
      isAppRunning: false,
      isLoading: false,
      error: null,
    }),
}));
