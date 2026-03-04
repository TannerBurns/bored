export { ChatView } from './ChatView';

import type { ChatMode } from '../../types';

export const MODE_BADGE_COLORS: Record<ChatMode, string> = {
  general: 'bg-blue-500/20 text-blue-400',
  spec_builder: 'bg-purple-500/20 text-purple-400',
  ticket_builder: 'bg-green-500/20 text-green-400',
  review: 'bg-orange-500/20 text-orange-400',
};

export const MODE_LABELS: Record<ChatMode, string> = {
  general: 'General',
  spec_builder: 'Spec Builder',
  ticket_builder: 'Ticket Builder',
  review: 'Review',
};
