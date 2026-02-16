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
const mockGetAgentHookScriptPath = vi.fn();
const mockInstallAgentHooksProject = vi.fn();
const mockInstallCommandsToProject = vi.fn();
const mockUpdateProjectHooks = vi.fn();
const mockGetAvailableAgents = vi.fn();

vi.mock('../../lib/tauri', () => ({
  getProjects: (...args: unknown[]) => mockGetProjects(...args),
  createProject: (...args: unknown[]) => mockCreateProject(...args),
  deleteProject: (...args: unknown[]) => mockDeleteProject(...args),
  browseForDirectory: (...args: unknown[]) => mockBrowseForDirectory(...args),
  checkGitStatus: (...args: unknown[]) => mockCheckGitStatus(...args),
  initGitRepo: (...args: unknown[]) => mockInitGitRepo(...args),
  createProjectFolder: (...args: unknown[]) => mockCreateProjectFolder(...args),
  getAgentHookScriptPath: (...args: unknown[]) => mockGetAgentHookScriptPath(...args),
  installAgentHooksProject: (...args: unknown[]) => mockInstallAgentHooksProject(...args),
  installCommandsToProject: (...args: unknown[]) => mockInstallCommandsToProject(...args),
  updateProjectHooks: (...args: unknown[]) => mockUpdateProjectHooks(...args),
  getAvailableAgents: (...args: unknown[]) => mockGetAvailableAgents(...args),
}));

// ── Test data ────────────────────────────────────────────────────────

const MOCK_AGENTS = [
  { id: 'cursor', displayName: 'Cursor', isAvailable: true, version: '1.0', brandColor: null },
  { id: 'claude', displayName: 'Claude', isAvailable: true, version: '1.0', brandColor: '#da7756' },
];

const MOCK_PROJECT = {
  id: 'proj-1',
  name: 'project',
  path: '/test/project',
  hooksInstalled: { cursor: false, claude: false },
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
  mockGetAvailableAgents.mockResolvedValue(MOCK_AGENTS);
  mockGetAgentHookScriptPath.mockResolvedValue('/app/scripts/hook.js');
  mockInstallAgentHooksProject.mockResolvedValue(undefined);
  mockInstallCommandsToProject.mockResolvedValue(undefined);
  mockUpdateProjectHooks.mockResolvedValue(undefined);
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

    it('installs hooks and commands for all agents', async () => {
      await renderAndWaitForLoad();
      await addExistingProjectViaUI();

      // Wait for the auto-setup to complete (getProjects called again after setup)
      await waitFor(() => {
        expect(mockGetProjects).toHaveBeenCalledTimes(2);
      });

      // Hooks installed for each agent
      expect(mockInstallAgentHooksProject).toHaveBeenCalledWith(
        'cursor', '/app/scripts/hook.js', '/test/project'
      );
      expect(mockInstallAgentHooksProject).toHaveBeenCalledWith(
        'claude', '/app/scripts/hook.js', '/test/project'
      );

      // Commands installed for each agent
      expect(mockInstallCommandsToProject).toHaveBeenCalledWith('cursor', '/test/project');
      expect(mockInstallCommandsToProject).toHaveBeenCalledWith('claude', '/test/project');

      // Per-agent hook status updated
      expect(mockUpdateProjectHooks).toHaveBeenCalledWith('proj-1', 'cursor', true);
      expect(mockUpdateProjectHooks).toHaveBeenCalledWith('proj-1', 'claude', true);
      expect(mockUpdateProjectHooks).toHaveBeenCalledTimes(2);
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
    it('shows warning when one agent hook install fails', async () => {
      setupHappyPathMocks();
      mockInstallAgentHooksProject.mockImplementation((agentId: string) => {
        if (agentId === 'cursor') return Promise.reject('permission denied');
        return Promise.resolve(undefined);
      });

      await renderAndWaitForLoad();
      await addExistingProjectViaUI();

      // Wait for setup to complete
      await waitFor(() => {
        expect(mockGetProjects).toHaveBeenCalledTimes(2);
      });

      // Warning message should be displayed
      await waitFor(() => {
        const errorEl = screen.getByText(/setup warnings/);
        expect(errorEl).toBeInTheDocument();
        expect(errorEl.textContent).toContain('Cursor hooks');
      });

      // Only claude's hook status was updated (cursor failed before updateProjectHooks)
      expect(mockUpdateProjectHooks).toHaveBeenCalledTimes(1);
      expect(mockUpdateProjectHooks).toHaveBeenCalledWith('proj-1', 'claude', true);
    });

    it('shows warning when hook script path is not available', async () => {
      setupHappyPathMocks();
      mockGetAgentHookScriptPath.mockImplementation((agentId: string) => {
        if (agentId === 'cursor') return Promise.resolve(null);
        return Promise.resolve('/app/scripts/hook.js');
      });

      await renderAndWaitForLoad();
      await addExistingProjectViaUI();

      await waitFor(() => {
        const errorEl = screen.getByText(/setup warnings/);
        expect(errorEl).toBeInTheDocument();
        expect(errorEl.textContent).toContain('hook script path not available');
      });

      // Only claude's hook status was updated (cursor had no path)
      expect(mockUpdateProjectHooks).toHaveBeenCalledTimes(1);
      expect(mockUpdateProjectHooks).toHaveBeenCalledWith('proj-1', 'claude', true);
    });

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

      // Both hooks still installed (command failure doesn't block hooks)
      expect(mockUpdateProjectHooks).toHaveBeenCalledTimes(2);
    });
  });

  describe('autoSetupProject — no agents', () => {
    it('completes without error when no agents are available', async () => {
      setupHappyPathMocks();
      mockGetAvailableAgents.mockResolvedValue([]);

      await renderAndWaitForLoad();
      await addExistingProjectViaUI();

      // Wait for setup to complete (getProjects reloaded after setup)
      await waitFor(() => {
        expect(mockGetProjects).toHaveBeenCalledTimes(2);
      });

      // No hook installs attempted
      expect(mockInstallAgentHooksProject).not.toHaveBeenCalled();
      expect(mockInstallCommandsToProject).not.toHaveBeenCalled();
      expect(mockUpdateProjectHooks).not.toHaveBeenCalled();
    });

    it('completes without error when getAvailableAgents fails', async () => {
      setupHappyPathMocks();
      mockGetAvailableAgents.mockRejectedValue(new Error('network error'));

      await renderAndWaitForLoad();
      await addExistingProjectViaUI();

      // Wait for setup to complete
      await waitFor(() => {
        expect(mockGetProjects).toHaveBeenCalledTimes(2);
      });

      expect(mockInstallAgentHooksProject).not.toHaveBeenCalled();
      expect(mockUpdateProjectHooks).not.toHaveBeenCalled();
    });
  });
});
