import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { toast } from 'sonner';
import { useSettingsStore } from '../stores/settingsStore';

interface TicketMovedEvent {
  ticketId: string;
  ticketTitle?: string;
  columnName: string;
  columnId: string;
}

const NOTIFIABLE_COLUMNS: Record<string, { title: string; description: (ticketTitle: string) => string }> = {
  Review: {
    title: 'Ready for Review',
    description: (t) => `"${t}" has completed work and is ready for your review.`,
  },
  Blocked: {
    title: 'Ticket Blocked',
    description: (t) => `"${t}" needs your attention — clarification or action required.`,
  },
};

export function useNotificationToasts(onOpenTicket?: (ticketId: string) => void) {
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setup = async () => {
      unlisten = await listen<TicketMovedEvent>('ticket-moved', (event) => {
        const { notificationsEnabled } = useSettingsStore.getState();
        if (!notificationsEnabled) return;

        const { ticketId, ticketTitle, columnName } = event.payload;
        const config = NOTIFIABLE_COLUMNS[columnName];
        if (!config) return;

        const title = ticketTitle || 'Ticket';

        toast(config.title, {
          description: config.description(title),
          action: onOpenTicket
            ? { label: 'View', onClick: () => onOpenTicket(ticketId) }
            : undefined,
          duration: 6000,
        });
      });
    };

    setup();
    return () => { unlisten?.(); };
  }, [onOpenTicket]);
}
