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

vi.mock('../../lib/tauri', () => ({
  getProjects: (...args: unknown[]) => mockGetProjects(...args),
  createProject: (...args: unknown[]) => mockCreateProject(...args),
  deleteProject: (...args: unknown[]) => mockDeleteProject(...args),
  browseForDirectory: (...args: unknown[]) => mockBrowseForDirectory(...args),
  checkGitStatus: (...args: unknown[]) => mockCheckGitStatus(...args),
  initGitRepo: (...args: unknown[]) => mockInitGitRepo(...args),
  createProjectFolder: (...args: unknown[]) => mockCreateProjectFolder(...args),
}));

// ── Test data ────────────────────────────────────────────────────────

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

function setupHappyPathMocks() {
  mockGetProjects.mockResolvedValue([]);
  mockBrowseForDirectory.mockResolvedValue('/test/project');
  mockCheckGitStatus.mockResolvedValue(true);
  mockCreateProject.mockResolvedValue(MOCK_PROJECT);
}

async function renderAndWaitForLoad() {
  render(<ProjectsList />);
  await waitFor(() => {
    expect(screen.queryByText('Loading projects...')).not.toBeInTheDocument();
  });
}

async function addExistingProjectViaUI() {
  fireEvent.click(screen.getByText('+ Add Existing'));
  fireEvent.click(screen.getByText('Browse'));

  await waitFor(() => {
    expect(screen.getByText('Add Project')).not.toBeDisabled();
  });

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

  describe('add existing project', () => {
    beforeEach(() => {
      setupHappyPathMocks();
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

    it('reloads project list after creation', async () => {
      await renderAndWaitForLoad();

      expect(mockGetProjects).toHaveBeenCalledTimes(1);

      await addExistingProjectViaUI();

      await waitFor(() => {
        expect(mockGetProjects).toHaveBeenCalledTimes(2);
      });
    });
  });
});
