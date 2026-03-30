import React from 'react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useTicketHandlers } from './useTicketHandlers';
import type { Ticket, Project } from '../types';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('../stores/boardStore', () => ({
  useBoardStore: Object.assign(
    (selector: (s: Record<string, unknown>) => unknown) =>
      selector({
        selectedTicket: null,
        openTicketModal: vi.fn(),
        closeTicketModal: vi.fn(),
        addComment: vi.fn(),
        updateComment: vi.fn(),
        createTicket: vi.fn(),
        updateTicket: vi.fn(),
        moveTicket: vi.fn(),
      }),
    {
      getState: vi.fn(() => ({
        selectedTicket: null,
      })),
      setState: vi.fn(),
    }
  ),
}));

const mockStartAgentRun = vi.fn();
const mockGetWorkspaceProjects = vi.fn();

vi.mock('../lib/tauri', () => ({
  deleteTicket: vi.fn(),
  startAgentRun: (...args: unknown[]) => mockStartAgentRun(...args),
  getWorkspaceProjects: (...args: unknown[]) => mockGetWorkspaceProjects(...args),
}));

const mockEnsureAgentConfigsSynced = vi.fn(() => Promise.resolve());

vi.mock('../stores/settingsStore', () => ({
  useSettingsStore: {
    getState: () => ({
      agentConfigs: {
        claude: {
          codeReviewMaxIterations: 3,
          stageTimeoutHours: 1,
          stageMaxRetries: 2,
          workflowStages: {},
        },
      },
    }),
  },
  ensureAgentConfigsSynced: () => mockEnsureAgentConfigsSynced(),
}));

vi.mock('../lib/logger', () => ({
  logger: {
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  },
}));

const makeTicket = (overrides: Partial<Ticket> = {}): Ticket => ({
  id: 'ticket-1',
  boardId: 'board-1',
  columnId: 'col-1',
  title: 'Test Ticket',
  descriptionMd: '',
  priority: 'medium',
  labels: [],
  createdAt: new Date('2024-01-01'),
  updatedAt: new Date('2024-01-01'),
  ...overrides,
});

const makeProject = (overrides: Partial<Project> = {}): Project => ({
  id: 'project-1',
  name: 'Test Project',
  path: '/test/project',
  allowShellCommands: false,
  allowFileWrites: true,
  blockedPatterns: [],
  settings: {},
  createdAt: new Date('2024-01-01'),
  updatedAt: new Date('2024-01-01'),
  ...overrides,
});

describe('useTicketHandlers — handleRunWithAgent', () => {
  const setTicketsMock = vi.fn();
  const setTickets = setTicketsMock as unknown as React.Dispatch<React.SetStateAction<Ticket[]>>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockStartAgentRun.mockResolvedValue('run-123');
    mockEnsureAgentConfigsSynced.mockResolvedValue(undefined);
  });

  const renderWith = (tickets: Ticket[], projects: Project[] = []) =>
    renderHook(() =>
      useTicketHandlers({ tickets, setTickets, projects })
    );

  it('returns error when ticket is not found', async () => {
    const { result } = renderWith([], []);

    const error = await result.current.handleRunWithAgent('missing-id', 'claude');

    expect(error).toBe('Ticket not found');
    expect(mockStartAgentRun).not.toHaveBeenCalled();
  });

  it('returns error when project is not found', async () => {
    const ticket = makeTicket({ projectId: 'proj-missing' });
    const { result } = renderWith([ticket], []);

    const error = await result.current.handleRunWithAgent('ticket-1', 'claude');

    expect(error).toBe('Project not found');
    expect(mockStartAgentRun).not.toHaveBeenCalled();
  });

  it('returns error when ticket has no project or workspace', async () => {
    const ticket = makeTicket({ projectId: undefined, workspaceId: undefined });
    const { result } = renderWith([ticket], []);

    const error = await result.current.handleRunWithAgent('ticket-1', 'claude');

    expect(error).toBe('Ticket has no project or workspace assigned');
    expect(mockStartAgentRun).not.toHaveBeenCalled();
  });

  it('returns error when workspace has no projects', async () => {
    const ticket = makeTicket({ projectId: undefined, workspaceId: 'ws-1' });
    mockGetWorkspaceProjects.mockResolvedValue([]);

    const { result } = renderWith([ticket], []);

    const error = await result.current.handleRunWithAgent('ticket-1', 'claude');

    expect(error).toBe('Workspace has no projects');
    expect(mockStartAgentRun).not.toHaveBeenCalled();
  });

  it('returns undefined on successful run start (project path)', async () => {
    const ticket = makeTicket({ projectId: 'project-1' });
    const project = makeProject({ id: 'project-1', path: '/repo' });

    const { result } = renderWith([ticket], [project]);

    const error = await result.current.handleRunWithAgent('ticket-1', 'claude');

    expect(error).toBeUndefined();
    expect(mockStartAgentRun).toHaveBeenCalledWith(
      'ticket-1',
      'claude',
      '/repo',
      expect.objectContaining({
        codeReviewMaxIterations: 3,
        stageTimeoutHours: 1,
        stageMaxRetries: 2,
      })
    );
  });

  it('returns undefined on successful run start (workspace path)', async () => {
    const ticket = makeTicket({ projectId: undefined, workspaceId: 'ws-1' });
    mockGetWorkspaceProjects.mockResolvedValue([
      makeProject({ id: 'ws-proj-1', path: '/ws/repo' }),
    ]);

    const { result } = renderWith([ticket], []);

    const error = await result.current.handleRunWithAgent('ticket-1', 'claude');

    expect(error).toBeUndefined();
    expect(mockStartAgentRun).toHaveBeenCalledWith(
      'ticket-1',
      'claude',
      '/ws/repo',
      expect.any(Object)
    );
  });

  it('returns error string when startAgentRun throws an Error', async () => {
    const ticket = makeTicket({ projectId: 'project-1' });
    const project = makeProject({ id: 'project-1' });
    mockStartAgentRun.mockRejectedValue(new Error('Worktree setup failed: git conflict'));

    const { result } = renderWith([ticket], [project]);

    const error = await result.current.handleRunWithAgent('ticket-1', 'claude');

    expect(error).toBe('Worktree setup failed: git conflict');
  });

  it('returns stringified error when startAgentRun throws a non-Error', async () => {
    const ticket = makeTicket({ projectId: 'project-1' });
    const project = makeProject({ id: 'project-1' });
    mockStartAgentRun.mockRejectedValue('raw string error');

    const { result } = renderWith([ticket], [project]);

    const error = await result.current.handleRunWithAgent('ticket-1', 'claude');

    expect(error).toBe('raw string error');
  });

  it('passes workflowMode to startAgentRun', async () => {
    const ticket = makeTicket({ projectId: 'project-1' });
    const project = makeProject({ id: 'project-1' });

    const { result } = renderWith([ticket], [project]);

    await result.current.handleRunWithAgent('ticket-1', 'claude', 'code_review_only');

    expect(mockStartAgentRun).toHaveBeenCalledWith(
      'ticket-1',
      'claude',
      expect.any(String),
      expect.objectContaining({ workflowMode: 'code_review_only' })
    );
  });

  it('updates tickets with lockedByRunId on success', async () => {
    const ticket = makeTicket({ projectId: 'project-1' });
    const project = makeProject({ id: 'project-1' });
    mockStartAgentRun.mockResolvedValue('new-run-id');

    const { result } = renderWith([ticket], [project]);

    await result.current.handleRunWithAgent('ticket-1', 'claude');

    expect(setTicketsMock).toHaveBeenCalled();
    const updater = setTicketsMock.mock.calls[0][0];
    const updated = updater([ticket]);
    expect(updated[0].lockedByRunId).toBe('new-run-id');
  });

  it('does not update tickets on failure', async () => {
    const ticket = makeTicket({ projectId: 'project-1' });
    const project = makeProject({ id: 'project-1' });
    mockStartAgentRun.mockRejectedValue(new Error('fail'));

    const { result } = renderWith([ticket], [project]);

    await result.current.handleRunWithAgent('ticket-1', 'claude');

    expect(setTicketsMock).not.toHaveBeenCalled();
  });
});
