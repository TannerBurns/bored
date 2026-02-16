import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useSSE } from './useSSE';
import { useBoardStore } from '../stores/boardStore';

// Capture the onmessage handler set by the hook
let capturedOnMessage: ((event: MessageEvent) => void) | null = null;

class MockEventSource {
  static OPEN = 1;
  readyState = MockEventSource.OPEN;
  onopen: (() => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: (() => void) | null = null;

  constructor(_url: string) {
    setTimeout(() => {
      this.onopen?.();
      capturedOnMessage = this.onmessage;
    }, 0);
  }

  close = vi.fn();
}

vi.stubGlobal('EventSource', MockEventSource);

const mockBoard = { id: 'board-1', name: 'Test', createdAt: new Date(), updatedAt: new Date() };
const mockTicket = {
  id: 'ticket-1',
  boardId: 'board-1',
  columnId: 'col-1',
  title: 'Test',
  descriptionMd: '',
  priority: 'medium' as const,
  labels: [],
  createdAt: new Date(),
  updatedAt: new Date(),
};

function simulateEvent(data: Record<string, unknown>) {
  capturedOnMessage?.({ data: JSON.stringify(data) } as MessageEvent);
}

describe('useSSE handleEvent task reloading', () => {
  const mockLoadBoardData = vi.fn();
  const mockLoadTasks = vi.fn();

  beforeEach(() => {
    vi.useFakeTimers();
    capturedOnMessage = null;
    mockLoadBoardData.mockReset();
    mockLoadTasks.mockReset();

    useBoardStore.setState({
      currentBoard: mockBoard,
      loadBoardData: mockLoadBoardData,
      loadTasks: mockLoadTasks,
      selectedTicket: mockTicket,
      isTicketModalOpen: true,
    } as never);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function renderAndConnect() {
    const hook = renderHook(() => useSSE('http://localhost:9111', 'tok'));
    vi.advanceTimersByTime(1);
    return hook;
  }

  it('ticket_moved reloads tasks when modal is open for that ticket', () => {
    renderAndConnect();
    simulateEvent({ type: 'ticket_moved', ticket_id: 'ticket-1' });

    expect(mockLoadTasks).toHaveBeenCalledWith('ticket-1');
  });

  it('ticket_moved does not reload tasks when modal is closed', () => {
    useBoardStore.setState({ isTicketModalOpen: false } as never);
    renderAndConnect();
    simulateEvent({ type: 'ticket_moved', ticket_id: 'ticket-1' });

    expect(mockLoadTasks).not.toHaveBeenCalled();
  });

  it('ticket_moved does not reload tasks for a different ticket', () => {
    renderAndConnect();
    simulateEvent({ type: 'ticket_moved', ticket_id: 'ticket-other' });

    expect(mockLoadTasks).not.toHaveBeenCalled();
  });

  it('ticket_updated reloads tasks when modal is open for that ticket', () => {
    renderAndConnect();
    simulateEvent({ type: 'ticket_updated', ticket_id: 'ticket-1' });

    expect(mockLoadTasks).toHaveBeenCalledWith('ticket-1');
  });

  it('run_completed reloads tasks when modal is open for that ticket', () => {
    renderAndConnect();
    simulateEvent({ type: 'run_completed', ticket_id: 'ticket-1' });

    expect(mockLoadTasks).toHaveBeenCalledWith('ticket-1');
  });

  it('run_completed does not reload tasks when modal is closed', () => {
    useBoardStore.setState({ isTicketModalOpen: false } as never);
    renderAndConnect();
    simulateEvent({ type: 'run_completed', ticket_id: 'ticket-1' });

    expect(mockLoadTasks).not.toHaveBeenCalled();
  });

  it('run_completed does not reload tasks for a different ticket', () => {
    renderAndConnect();
    simulateEvent({ type: 'run_completed', ticket_id: 'ticket-other' });

    expect(mockLoadTasks).not.toHaveBeenCalled();
  });

  it('comment_added does not reload tasks', () => {
    renderAndConnect();
    simulateEvent({ type: 'comment_added', ticket_id: 'ticket-1' });

    expect(mockLoadTasks).not.toHaveBeenCalled();
  });

  it('does not reload tasks when no ticket is selected', () => {
    useBoardStore.setState({ selectedTicket: null } as never);
    renderAndConnect();
    simulateEvent({ type: 'ticket_moved', ticket_id: 'ticket-1' });

    expect(mockLoadTasks).not.toHaveBeenCalled();
  });
});
