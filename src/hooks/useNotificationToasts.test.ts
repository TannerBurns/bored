import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useNotificationToasts } from './useNotificationToasts';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

const mockToast = vi.fn();
vi.mock('sonner', () => ({
  toast: (...args: unknown[]) => mockToast(...args),
}));

type ListenCallback = (event: { payload: Record<string, unknown> }) => void;
let capturedListener: ListenCallback | null = null;
const mockUnlisten = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((eventName: string, callback: ListenCallback) => {
    if (eventName === 'ticket-moved') {
      capturedListener = callback;
    }
    return Promise.resolve(mockUnlisten);
  }),
}));

const mockGetState = vi.fn();
vi.mock('../stores/settingsStore', () => ({
  useSettingsStore: {
    getState: () => mockGetState(),
  },
}));

function fireTicketMoved(payload: Record<string, unknown>) {
  expect(capturedListener).not.toBeNull();
  capturedListener!({ payload });
}

describe('useNotificationToasts', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedListener = null;
    mockGetState.mockReturnValue({ notificationsEnabled: true });
  });

  it('registers a ticket-moved listener on mount', async () => {
    const { listen } = await import('@tauri-apps/api/event');
    renderHook(() => useNotificationToasts());

    await act(async () => {});

    expect(listen).toHaveBeenCalledWith('ticket-moved', expect.any(Function));
  });

  it('unlistens on unmount', async () => {
    const { unmount } = renderHook(() => useNotificationToasts());
    await act(async () => {});

    unmount();
    expect(mockUnlisten).toHaveBeenCalled();
  });

  describe('Review column', () => {
    it('shows toast with correct title and description', async () => {
      renderHook(() => useNotificationToasts());
      await act(async () => {});

      fireTicketMoved({
        ticketId: 't-1',
        ticketTitle: 'Fix login bug',
        columnName: 'Review',
        columnId: 'col-review',
      });

      expect(mockToast).toHaveBeenCalledWith('Ready for Review', expect.objectContaining({
        description: '"Fix login bug" has completed work and is ready for your review.',
      }));
    });
  });

  describe('Blocked column', () => {
    it('shows toast with correct title and description', async () => {
      renderHook(() => useNotificationToasts());
      await act(async () => {});

      fireTicketMoved({
        ticketId: 't-2',
        ticketTitle: 'Add dark mode',
        columnName: 'Blocked',
        columnId: 'col-blocked',
      });

      expect(mockToast).toHaveBeenCalledWith('Ticket Blocked', expect.objectContaining({
        description: '"Add dark mode" needs your attention — clarification or action required.',
      }));
    });
  });

  describe('non-notifiable columns', () => {
    it.each(['In Progress', 'Todo', 'Done', 'Backlog'])(
      'does not show toast for "%s"',
      async (columnName) => {
        renderHook(() => useNotificationToasts());
        await act(async () => {});

        fireTicketMoved({
          ticketId: 't-3',
          ticketTitle: 'Some ticket',
          columnName,
          columnId: 'col-x',
        });

        expect(mockToast).not.toHaveBeenCalled();
      }
    );
  });

  describe('notificationsEnabled setting', () => {
    it('does not show toast when notifications are disabled', async () => {
      mockGetState.mockReturnValue({ notificationsEnabled: false });

      renderHook(() => useNotificationToasts());
      await act(async () => {});

      fireTicketMoved({
        ticketId: 't-4',
        ticketTitle: 'Some ticket',
        columnName: 'Review',
        columnId: 'col-review',
      });

      expect(mockToast).not.toHaveBeenCalled();
    });

    it('shows toast when notifications are re-enabled', async () => {
      mockGetState.mockReturnValue({ notificationsEnabled: false });

      renderHook(() => useNotificationToasts());
      await act(async () => {});

      fireTicketMoved({
        ticketId: 't-5',
        ticketTitle: 'Ticket A',
        columnName: 'Review',
        columnId: 'col-review',
      });
      expect(mockToast).not.toHaveBeenCalled();

      mockGetState.mockReturnValue({ notificationsEnabled: true });

      fireTicketMoved({
        ticketId: 't-6',
        ticketTitle: 'Ticket B',
        columnName: 'Review',
        columnId: 'col-review',
      });
      expect(mockToast).toHaveBeenCalledTimes(1);
    });
  });

  describe('ticketTitle fallback', () => {
    it('uses "Ticket" when ticketTitle is missing', async () => {
      renderHook(() => useNotificationToasts());
      await act(async () => {});

      fireTicketMoved({
        ticketId: 't-7',
        columnName: 'Review',
        columnId: 'col-review',
      });

      expect(mockToast).toHaveBeenCalledWith('Ready for Review', expect.objectContaining({
        description: '"Ticket" has completed work and is ready for your review.',
      }));
    });

    it('uses "Ticket" when ticketTitle is empty string', async () => {
      renderHook(() => useNotificationToasts());
      await act(async () => {});

      fireTicketMoved({
        ticketId: 't-8',
        ticketTitle: '',
        columnName: 'Blocked',
        columnId: 'col-blocked',
      });

      expect(mockToast).toHaveBeenCalledWith('Ticket Blocked', expect.objectContaining({
        description: '"Ticket" needs your attention — clarification or action required.',
      }));
    });
  });

  describe('onOpenTicket action', () => {
    it('includes View action when onOpenTicket is provided', async () => {
      const onOpen = vi.fn();
      renderHook(() => useNotificationToasts(onOpen));
      await act(async () => {});

      fireTicketMoved({
        ticketId: 't-9',
        ticketTitle: 'My ticket',
        columnName: 'Review',
        columnId: 'col-review',
      });

      const callArgs = mockToast.mock.calls[0][1];
      expect(callArgs.action).toEqual({
        label: 'View',
        onClick: expect.any(Function),
      });

      callArgs.action.onClick();
      expect(onOpen).toHaveBeenCalledWith('t-9');
    });

    it('has no action when onOpenTicket is undefined', async () => {
      renderHook(() => useNotificationToasts());
      await act(async () => {});

      fireTicketMoved({
        ticketId: 't-10',
        ticketTitle: 'My ticket',
        columnName: 'Review',
        columnId: 'col-review',
      });

      const callArgs = mockToast.mock.calls[0][1];
      expect(callArgs.action).toBeUndefined();
    });
  });

  describe('toast options', () => {
    it('sets duration to 6000ms', async () => {
      renderHook(() => useNotificationToasts());
      await act(async () => {});

      fireTicketMoved({
        ticketId: 't-11',
        ticketTitle: 'Test',
        columnName: 'Review',
        columnId: 'col-review',
      });

      const callArgs = mockToast.mock.calls[0][1];
      expect(callArgs.duration).toBe(6000);
    });
  });
});
