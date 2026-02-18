import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ProjectsList } from './ProjectsList';

// ── Tauri mocks ──────────────────────────────────────────────────────

const mockGetProjects = vi.fn();
const mockCreateProject = vi.fn();
const mockDeleteProject = vi.fn();
const mockBrowseForDirectory = vi.fn();
const mockCheckGitStatus = vi.fn();
const mockInitGitRepo = vi.fn();
const mockCreateProjectFolder = vi.fn();
const mockInstallCommandsToProject = vi.fn();

vi.mock('../../lib/tauri', () => ({
  getProjects: (...args: unknown[]) => mockGetProjects(...args),
  createProject: (...args: unknown[]) => mockCreateProject(...args),
  deleteProject: (...args: unknown[]) => mockDeleteProject(...args),
  browseForDirectory: (...args: unknown[]) => mockBrowseForDirectory(...args),
  checkGitStatus: (...args: unknown[]) => mockCheckGitStatus(...args),
  initGitRepo: (...args: unknown[]) => mockInitGitRepo(...args),
  createProjectFolder: (...args: unknown[]) => mockCreateProjectFolder(...args),
  installCommandsToProject: (...args: unknown[]) => mockInstallCommandsToProject(...args),
}));

// ── Test data ────────────────────────────────────────────────────────

const MOCK_AGENTS = [
  { id: 'cursor', displayName: 'Cursor', isAvailable: true, version: '1.0', brandColor: null },
  { id: 'claude', displayName: 'Claude', isAvailable: true, version: '1.0', brandColor: '#da7756' },
];

// Mock the agent registry store
let storeAgents = MOCK_AGENTS;
vi.mock('../../stores/agentRegistryStore', () => ({
  useAgentRegistryStore: {
    getState: () => ({ agents: storeAgents }),
  },
}));

const MOCK_PROJECT = {
  id: 'proj-1',
  name: 'project',
  path: '/test/project',
  allowShellCommands: true,
  allowFileWrites: true,
  blockedPatterns: [],
  settings: {},
};

// ── Helpers ──────────────────────────────────────────────────────────

/** Set up default mocks for a successful "add existing project" flow. */
function setupHappyPathMocks() {
  mockGetProjects.mockResolvedValue([]);
  mockBrowseForDirectory.mockResolvedValue('/test/project');
  mockCheckGitStatus.mockResolvedValue(true);
  mockCreateProject.mockResolvedValue(MOCK_PROJECT);
  storeAgents = MOCK_AGENTS;
  mockInstallCommandsToProject.mockResolvedValue(undefined);
}

/** Renders the component and waits for the initial load to finish. */
async function renderAndWaitForLoad() {
  render(<ProjectsList />);
  await waitFor(() => {
    expect(screen.queryByText('Loading projects...')).not.toBeInTheDocument();
  });
}

/**
 * Drive the "add existing" form through Browse -> Add Project.
 * Assumes the component is already rendered and loaded.
 */
async function addExistingProjectViaUI() {
  // Open the "Add Existing" form
  fireEvent.click(screen.getByText('+ Add Existing'));

  // Click Browse to trigger path selection + git check
  fireEvent.click(screen.getByText('Browse'));

  // Wait for git check to resolve and button to become enabled
  await waitFor(() => {
    expect(screen.getByText('Add Project')).not.toBeDisabled();
  });

  // Click "Add Project" to trigger create + auto-setup
  fireEvent.click(screen.getByText('Add Project'));
}

// ── Tests ────────────────────────────────────────────────────────────

describe('ProjectsList', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders empty state when no projects exist', async () => {
    mockGetProjects.mockResolvedValue([]);
    await renderAndWaitForLoad();
    expect(screen.getByText('No projects added yet.')).toBeInTheDocument();
  });

  it('renders existing projects', async () => {
    mockGetProjects.mockResolvedValue([MOCK_PROJECT]);
    await renderAndWaitForLoad();
    expect(screen.getByText('project')).toBeInTheDocument();
    expect(screen.getByText('/test/project')).toBeInTheDocument();
  });

  describe('autoSetupProject — happy path', () => {
    beforeEach(() => {
      setupHappyPathMocks();
    });

    it('installs commands for all agents', async () => {
      await renderAndWaitForLoad();
      await addExistingProjectViaUI();

      // Wait for the auto-setup to complete (getProjects called again after setup)
      await waitFor(() => {
        expect(mockGetProjects).toHaveBeenCalledTimes(2);
      });

      // Commands installed for each agent
      expect(mockInstallCommandsToProject).toHaveBeenCalledWith('cursor', '/test/project');
      expect(mockInstallCommandsToProject).toHaveBeenCalledWith('claude', '/test/project');
    });

    it('calls createProject with browsed name and path', async () => {
      await renderAndWaitForLoad();
      await addExistingProjectViaUI();

      await waitFor(() => {
        expect(mockCreateProject).toHaveBeenCalled();
      });

      expect(mockCreateProject).toHaveBeenCalledWith({
        name: 'project',
        path: '/test/project',
      });
    });

    it('reloads project list after successful setup', async () => {
      await renderAndWaitForLoad();

      // First call during mount
      expect(mockGetProjects).toHaveBeenCalledTimes(1);

      await addExistingProjectViaUI();

      // Second call after setup completes
      await waitFor(() => {
        expect(mockGetProjects).toHaveBeenCalledTimes(2);
      });
    });
  });

  describe('autoSetupProject — partial failure', () => {
    it('shows warning when command install fails', async () => {
      setupHappyPathMocks();
      mockInstallCommandsToProject.mockImplementation((agentId: string) => {
        if (agentId === 'claude') return Promise.reject('write error');
        return Promise.resolve(undefined);
      });

      await renderAndWaitForLoad();
      await addExistingProjectViaUI();

      await waitFor(() => {
        const errorEl = screen.getByText(/setup warnings/);
        expect(errorEl).toBeInTheDocument();
        expect(errorEl.textContent).toContain('Claude commands');
      });
    });
  });

  describe('autoSetupProject — no agents', () => {
    it('completes without error when no agents are available', async () => {
      setupHappyPathMocks();
      storeAgents = [];

      await renderAndWaitForLoad();
      await addExistingProjectViaUI();

      // Wait for setup to complete (getProjects reloaded after setup)
      await waitFor(() => {
        expect(mockGetProjects).toHaveBeenCalledTimes(2);
      });

      // No command installs attempted
      expect(mockInstallCommandsToProject).not.toHaveBeenCalled();
    });
  });
});
