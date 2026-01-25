import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { EventTimeline } from './EventTimeline';

vi.mock('../../lib/tauri', () => ({
  getRunEvents: vi.fn(),
}));

import { getRunEvents } from '../../lib/tauri';

const mockEvents = [
  {
    id: 'evt-1',
    runId: 'run-1',
    ticketId: 'ticket-1',
    eventType: 'command_executed',
    payload: {
      raw: '{"command":"ls"}',
      structured: { command: 'ls -la' },
    },
    createdAt: new Date().toISOString(),
  },
  {
    id: 'evt-2',
    runId: 'run-1',
    ticketId: 'ticket-1',
    eventType: 'file_edited',
    payload: {
      structured: { filePath: 'src/index.ts', tool: 'edit' },
    },
    createdAt: new Date(Date.now() - 60000).toISOString(),
  },
];

describe('EventTimeline', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading state initially', () => {
    vi.mocked(getRunEvents).mockImplementation(() => new Promise(() => {}));
    const { container } = render(<EventTimeline runId="run-1" />);
    expect(container.querySelector('.animate-spin')).toBeTruthy();
  });

  it('displays events after loading', async () => {
    vi.mocked(getRunEvents).mockResolvedValue(mockEvents);
    render(<EventTimeline runId="run-1" />);

    await waitFor(() => {
      expect(screen.getByText('command executed')).toBeInTheDocument();
    });
    expect(screen.getByText('file edited')).toBeInTheDocument();
    expect(screen.getByText('ls -la')).toBeInTheDocument();
  });

  it('shows empty state when no events', async () => {
    vi.mocked(getRunEvents).mockResolvedValue([]);
    render(<EventTimeline runId="run-1" />);

    await waitFor(() => {
      expect(screen.getByText('No events yet')).toBeInTheDocument();
    });
  });

  it('shows error state on failure', async () => {
    vi.mocked(getRunEvents).mockRejectedValue(new Error('Network error'));
    render(<EventTimeline runId="run-1" />);

    await waitFor(() => {
      expect(screen.getByText('Error loading events')).toBeInTheDocument();
    });
  });

  it('displays file path for file events', async () => {
    vi.mocked(getRunEvents).mockResolvedValue([mockEvents[1]]);
    render(<EventTimeline runId="run-1" />);

    await waitFor(() => {
      expect(screen.getByText('src/index.ts')).toBeInTheDocument();
    });
  });

  it('calls getRunEvents with correct run id', async () => {
    vi.mocked(getRunEvents).mockResolvedValue([]);
    render(<EventTimeline runId="test-run-123" />);

    await waitFor(() => {
      expect(getRunEvents).toHaveBeenCalledWith('test-run-123');
    });
  });
});
