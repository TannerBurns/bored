import { create } from 'zustand';
import type { ValidationSession, ValidationMessage } from '../types';
import {
  createValidationSession as apiCreateSession,
  getValidationSession as apiGetSession,
  getValidationSessions as apiGetSessions,
  getValidationMessages as apiGetMessages,
  sendValidationMessage as apiSendMessage,
  stopValidationApp as apiStopValidationApp,
  updateValidationSessionStatus as apiUpdateStatus,
  deleteValidationSession as apiDeleteSession,
  pushBranch as apiPushBranch,
  createPullRequest as apiCreatePR,
  getBranchDiffFiles as apiGetDiffFiles,
} from '../lib/tauri';
import { useSettingsStore } from './settingsStore';
import type { PushResult, PullRequestResult, FileDiff } from '../types';
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

  // Agent thinking logs (processed CLI output for current session)
  agentLogs: string[];

  // App subprocess logs (e.g. npm run dev stdout/stderr)
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
    agentType?: 'cursor' | 'claude';
  }) => Promise<ValidationSession>;
  selectSession: (session: ValidationSession | null) => void;
  updateSessionStatus: (sessionId: string, status: string) => Promise<void>;
  deleteSession: (sessionId: string) => Promise<void>;
  refreshSession: (sessionId: string) => Promise<void>;

  // Chat actions
  loadMessages: (sessionId: string) => Promise<void>;
  sendMessage: (sessionId: string, content: string) => Promise<ValidationMessage>;

  // Agent log actions (thinking block)
  addAgentLog: (message: string) => void;

  // App log actions
  addAppLogs: (logs: AppLogEntry[]) => void;
  clearAppLogs: () => void;
  stopApp: (sessionId: string) => Promise<void>;

  // Next steps actions
  pushBranch: (ticketId: string) => Promise<PushResult>;
  createPullRequest: (ticketId: string, title?: string, body?: string) => Promise<PullRequestResult>;
  getBranchDiffFiles: (ticketId: string) => Promise<FileDiff[]>;

  // Reset
  reset: () => void;
}

export const useValidationStore = create<ValidationState>((set) => ({
  // Initial state
  sessions: [],
  currentSession: null,
  messages: [],
  isAgentThinking: false,
  agentLogs: [],
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
      agentLogs: [],
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
      set({ isAgentThinking: true, agentLogs: [] });
      const { validationModel, validationTimeoutMinutes } = useSettingsStore.getState();
      const message = await apiSendMessage(sessionId, content, {
        model: validationModel,
        timeoutMinutes: validationTimeoutMinutes,
      });
      // Reload all messages from DB to get full content (replaces SSE placeholders)
      const messages = await apiGetMessages(sessionId);
      set({
        messages,
        isAgentThinking: false,
      });
      return message;
    } catch (e) {
      logger.error('Failed to send validation message', e);
      set({ isAgentThinking: false, error: String(e) });
      throw e;
    }
  },

  // Agent log actions (thinking block)
  addAgentLog: (message) => {
    set((state) => ({
      agentLogs: [...state.agentLogs, message],
    }));
  },

  // App log actions
  addAppLogs: (logs) => {
    if (logs.length === 0) return;
    set((state) => {
      const MAX_APP_LOGS = 500;
      const next = [...state.appLogs, ...logs];
      return { appLogs: next.length > MAX_APP_LOGS ? next.slice(-MAX_APP_LOGS) : next };
    });
  },

  clearAppLogs: () => set({ appLogs: [] }),

  stopApp: async (sessionId) => {
    try {
      await apiStopValidationApp(sessionId);
      const session = await apiGetSession(sessionId);
      set((state) => ({
        currentSession:
          state.currentSession?.id === sessionId ? session : state.currentSession,
        sessions: state.sessions.map((s) =>
          s.id === sessionId ? session : s
        ),
      }));
    } catch (e) {
      logger.error('Failed to stop validation app', e);
      set({ error: String(e) });
    }
  },

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

  getBranchDiffFiles: async (ticketId) => {
    try {
      return await apiGetDiffFiles(ticketId);
    } catch (e) {
      set({ error: String(e) });
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
      agentLogs: [],
      appLogs: [],
      isAppRunning: false,
      isLoading: false,
      error: null,
    }),
}));
