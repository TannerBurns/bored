import { useState } from 'react';
import { createTicketsFromChat } from '../../lib/tauri';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { useChatStore } from '../../stores/chatStore';

interface TicketBuilderMessageProps {
  content: string;
  chatId: string;
  alreadyCreated: boolean;
}

interface TicketBuilderParsed {
  tickets: ParsedTicket[];
  textBefore: string;
  textAfter: string;
}

interface ParsedTicket {
  title: string;
  description: string;
  priority?: string;
  tasks?: { title: string; content?: string }[];
}

const PRIORITY_COLORS: Record<string, string> = {
  low: 'bg-green-500/20 text-green-400',
  medium: 'bg-yellow-500/20 text-yellow-400',
  high: 'bg-orange-500/20 text-orange-400',
  urgent: 'bg-red-500/20 text-red-400',
};

function tryParseJson(text: string): { tickets: ParsedTicket[] } | null {
  try {
    const parsed = JSON.parse(text);
    if (parsed?.tickets && Array.isArray(parsed.tickets)) return parsed;
  } catch { /* fall through */ }

  try {
    const repaired = repairUnquotedValues(text);
    const parsed = JSON.parse(repaired);
    if (parsed?.tickets && Array.isArray(parsed.tickets)) return parsed;
  } catch { /* fall through */ }

  return null;
}

function repairUnquotedValues(text: string): string {
  return text.replace(
    /"(content|title|description)":\s+([A-Za-z])/g,
    '"$1": "$2',
  );
}

function parseTicketBuilderResponse(content: string): TicketBuilderParsed | null {
  const jsonMatch = content.match(/```json\s*([\s\S]*?)```/);
  if (jsonMatch) {
    const parsed = tryParseJson(jsonMatch[1]);
    if (parsed) {
      const jsonStart = content.indexOf(jsonMatch[0]);
      return {
        tickets: parsed.tickets,
        textBefore: content.slice(0, jsonStart).trim(),
        textAfter: content.slice(jsonStart + jsonMatch[0].length).trim(),
      };
    }
  }

  const rawMatch = content.match(/\{[\s\S]*"tickets"[\s\S]*\}/);
  if (rawMatch) {
    const parsed = tryParseJson(rawMatch[0]);
    if (parsed) {
      return {
        tickets: parsed.tickets,
        textBefore: content.slice(0, rawMatch.index).trim(),
        textAfter: content.slice(rawMatch.index! + rawMatch[0].length).trim(),
      };
    }
  }

  return null;
}

export function TicketBuilderMessage({ content, chatId, alreadyCreated }: TicketBuilderMessageProps) {
  const parsed = parseTicketBuilderResponse(content);
  const [isCreating, setIsCreating] = useState(false);
  const [justCreated, setJustCreated] = useState(false);
  const loadMessages = useChatStore((s) => s.loadMessages);
  const created = alreadyCreated || justCreated;

  if (!parsed) {
    return (
      <div className="rounded-xl px-4 py-2.5 glass">
        <MarkdownViewer content={content} />
      </div>
    );
  }

  const handleCreateTickets = async () => {
    setIsCreating(true);
    try {
      const ticketsJson = JSON.stringify({ tickets: parsed.tickets });
      await createTicketsFromChat(chatId, ticketsJson);
      setJustCreated(true);
      await loadMessages(chatId);
    } catch (e) {
      console.error('Failed to create tickets:', e);
    } finally {
      setIsCreating(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="space-y-3">
        {parsed.tickets.map((ticket, i) => (
          <TicketPreviewCard key={i} ticket={ticket} />
        ))}
      </div>

      {!created && (
        <button
          onClick={handleCreateTickets}
          disabled={isCreating}
          className="px-4 py-2 bg-status-info text-white rounded-lg hover:bg-status-info/90 disabled:opacity-50 disabled:cursor-not-allowed text-sm font-medium transition-colors"
        >
          {isCreating
            ? 'Creating...'
            : `Create ${parsed.tickets.length} Ticket${parsed.tickets.length !== 1 ? 's' : ''}`}
        </button>
      )}

      {created && (
        <div className="text-sm text-green-400 flex items-center gap-1.5">
          <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
            <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
          </svg>
          Tickets created
        </div>
      )}
    </div>
  );
}

function TicketPreviewCard({ ticket }: { ticket: ParsedTicket }) {
  const [expanded, setExpanded] = useState(false);
  const priority = ticket.priority || 'medium';
  const colorClass = PRIORITY_COLORS[priority] || PRIORITY_COLORS.medium;

  return (
    <div className="border border-board-border rounded-lg p-4 bg-board-card/30">
      <div className="flex items-center gap-2 mb-2">
        <span className={`px-2 py-0.5 rounded text-xs font-medium ${colorClass}`}>
          {priority}
        </span>
        <h4 className="font-medium text-board-text">{ticket.title}</h4>
      </div>

      <div className="text-sm text-board-text-muted">
        <button
          onClick={() => setExpanded(!expanded)}
          className="text-xs text-board-accent hover:underline"
        >
          {expanded ? 'Hide description' : 'Show description'}
        </button>
        {expanded && (
          <div className="mt-2">
            <MarkdownViewer content={ticket.description} />
          </div>
        )}
      </div>

      {ticket.tasks && ticket.tasks.length > 0 && (
        <div className="text-xs text-board-text-muted mt-2 flex items-center gap-1">
          <svg xmlns="http://www.w3.org/2000/svg" className="h-3.5 w-3.5" viewBox="0 0 20 20" fill="currentColor">
            <path d="M9 2a1 1 0 000 2h2a1 1 0 100-2H9z" />
            <path fillRule="evenodd" d="M4 5a2 2 0 012-2 3 3 0 003 3h2a3 3 0 003-3 2 2 0 012 2v11a2 2 0 01-2 2H6a2 2 0 01-2-2V5zm3 4a1 1 0 000 2h.01a1 1 0 100-2H7zm3 0a1 1 0 000 2h3a1 1 0 100-2h-3zm-3 4a1 1 0 100 2h.01a1 1 0 100-2H7zm3 0a1 1 0 100 2h3a1 1 0 100-2h-3z" clipRule="evenodd" />
          </svg>
          <span>{ticket.tasks.length} task{ticket.tasks.length !== 1 ? 's' : ''}: {ticket.tasks.map(t => t.title).join(', ')}</span>
        </div>
      )}
    </div>
  );
}
