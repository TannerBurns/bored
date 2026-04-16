import { useState } from 'react';
import { createTicketsFromChat } from '../../lib/tauri';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { useChatStore } from '../../stores/chatStore';

interface TicketBuilderMessageProps {
  content: string;
  chatId: string;
  alreadyCreated: boolean;
}

interface ParsedEpic {
  id?: string;
  name?: string;
  description?: string;
  tickets: ParsedTicket[];
}

interface ParsedUpdate {
  ticket_id: string;
  title?: string;
  description?: string;
  priority?: string;
  tasks?: { title: string; content?: string }[];
}

interface TicketBuilderParsed {
  tickets: ParsedTicket[];
  epics: ParsedEpic[];
  updates: ParsedUpdate[];
  textBefore: string;
  textAfter: string;
}

interface ParsedTicket {
  id?: string;
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

function hasTicketData(obj: Record<string, unknown>): boolean {
  return (
    (Array.isArray(obj?.tickets) && obj.tickets.length > 0) ||
    (Array.isArray(obj?.epics) && obj.epics.length > 0) ||
    (Array.isArray(obj?.updates) && obj.updates.length > 0)
  );
}

type ParsedOutput = { tickets: ParsedTicket[]; epics: ParsedEpic[]; updates: ParsedUpdate[] };

function tryParseJson(text: string): ParsedOutput | null {
  try {
    const parsed = JSON.parse(text);
    if (hasTicketData(parsed)) return { tickets: parsed.tickets ?? [], epics: parsed.epics ?? [], updates: parsed.updates ?? [] };
  } catch { /* fall through */ }

  try {
    const repaired = repairUnquotedValues(text);
    const parsed = JSON.parse(repaired);
    if (hasTicketData(parsed)) return { tickets: parsed.tickets ?? [], epics: parsed.epics ?? [], updates: parsed.updates ?? [] };
  } catch { /* fall through */ }

  return null;
}

function repairUnquotedValues(text: string): string {
  return text.replace(
    /"(content|title|description|name|ticket_id|id)":\s+([A-Za-z])/g,
    '"$1": "$2',
  );
}

/** Walk from the opening brace and find the matching closing brace,
 *  correctly skipping braces inside JSON string literals so that embedded
 *  markdown code blocks (```json ... ```) inside description values don't
 *  break extraction. */
function extractBalancedJson(text: string): string | null {
  let depth = 0;
  let inString = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (inString) {
      if (ch === '\\') { i++; continue; }
      if (ch === '"') inString = false;
      continue;
    }
    if (ch === '"') { inString = true; continue; }
    if (ch === '{') depth++;
    else if (ch === '}') {
      depth--;
      if (depth === 0) return text.slice(0, i + 1);
    }
  }
  return null;
}

function parseTicketBuilderResponse(content: string): TicketBuilderParsed | null {
  const buildResult = (parsed: ParsedOutput, before: string, after: string): TicketBuilderParsed => ({
    tickets: parsed.tickets,
    epics: parsed.epics,
    updates: parsed.updates,
    textBefore: before,
    textAfter: after,
  });

  const fenceIdx = content.indexOf('```json');
  if (fenceIdx !== -1) {
    const afterFence = content.slice(fenceIdx + 7);
    const braceOffset = afterFence.indexOf('{');
    if (braceOffset !== -1) {
      const candidate = extractBalancedJson(afterFence.slice(braceOffset));
      if (candidate) {
        const parsed = tryParseJson(candidate);
        if (parsed) {
          const jsonEnd = fenceIdx + 7 + braceOffset + candidate.length;
          const rest = content.slice(jsonEnd);
          const closingFenceIdx = rest.indexOf('```');
          const blockEnd = closingFenceIdx !== -1 ? jsonEnd + closingFenceIdx + 3 : jsonEnd;
          return buildResult(parsed, content.slice(0, fenceIdx).trim(), content.slice(blockEnd).trim());
        }
      }
    }
  }

  const indices = [
    content.indexOf('"tickets"'),
    content.indexOf('"epics"'),
    content.indexOf('"updates"'),
  ].filter((i) => i !== -1);
  const anchorIdx = indices.length > 0 ? Math.min(...indices) : -1;

  if (anchorIdx !== -1) {
    let searchFrom = anchorIdx;
    while (searchFrom >= 0) {
      const braceIdx = content.lastIndexOf('{', searchFrom);
      if (braceIdx === -1) break;
      const candidate = extractBalancedJson(content.slice(braceIdx));
      if (candidate) {
        const parsed = tryParseJson(candidate);
        if (parsed) {
          return buildResult(parsed, content.slice(0, braceIdx).trim(), content.slice(braceIdx + candidate.length).trim());
        }
      }
      searchFrom = braceIdx - 1;
    }
  }

  return null;
}

function buildActionLabel(parsed: TicketBuilderParsed): string {
  const newEpicCount = parsed.epics.filter((e) => !e.id).length;
  const linkToEpicCount = parsed.epics.reduce(
    (sum, e) => sum + e.tickets.filter((t) => t.id).length,
    0,
  );
  const newTicketsUnderEpics = parsed.epics.reduce(
    (sum, e) => sum + e.tickets.filter((t) => !t.id).length,
    0,
  );
  const standaloneCount = parsed.tickets.length;
  const newTicketCreates = newTicketsUnderEpics + standaloneCount;
  const updateCount = parsed.updates.length;

  const parts: string[] = [];

  if (newEpicCount > 0 && newTicketCreates > 0) {
    parts.push(
      `Create ${newEpicCount} Epic${newEpicCount !== 1 ? 's' : ''} with ${newTicketCreates} Ticket${newTicketCreates !== 1 ? 's' : ''}`,
    );
  } else if (newTicketCreates > 0) {
    parts.push(`Create ${newTicketCreates} Ticket${newTicketCreates !== 1 ? 's' : ''}`);
  }

  if (linkToEpicCount > 0) {
    parts.push(
      `Link ${linkToEpicCount} existing ticket${linkToEpicCount !== 1 ? 's' : ''} to epic`,
    );
  }

  if (updateCount > 0) {
    parts.push(`Update ${updateCount} Ticket${updateCount !== 1 ? 's' : ''}`);
  }

  return parts.join(' and ') || 'Apply';
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

  const handleApply = async () => {
    setIsCreating(true);
    try {
      const ticketsJson = JSON.stringify({
        tickets: parsed.tickets,
        epics: parsed.epics,
        updates: parsed.updates,
      });
      await createTicketsFromChat(chatId, ticketsJson);
      setJustCreated(true);
      await loadMessages(chatId);
    } catch (e) {
      console.error('Failed to apply ticket changes:', e);
    } finally {
      setIsCreating(false);
    }
  };

  return (
    <div className="space-y-3">
      {parsed.textBefore && (
        <div className="rounded-xl px-4 py-2.5 glass">
          <MarkdownViewer content={parsed.textBefore} />
        </div>
      )}

      <div className="space-y-3">
        {parsed.epics.map((epic, i) => (
          <EpicPreviewCard key={`epic-${i}`} epic={epic} index={i} />
        ))}
        {parsed.tickets.map((ticket, i) => (
          <TicketPreviewCard key={`ticket-${i}`} ticket={ticket} />
        ))}
        {parsed.updates.map((update, i) => (
          <TicketUpdatePreviewCard key={`update-${i}`} update={update} />
        ))}
      </div>

      {!created && (
        <button
          onClick={handleApply}
          disabled={isCreating}
          className="px-4 py-2 bg-status-info text-white rounded-lg hover:bg-status-info/90 disabled:opacity-50 disabled:cursor-not-allowed text-sm font-medium transition-colors"
        >
          {isCreating ? 'Applying...' : buildActionLabel(parsed)}
        </button>
      )}

      {created && (
        <div className="text-sm text-green-400 flex items-center gap-1.5">
          <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
            <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
          </svg>
          Changes applied
        </div>
      )}

      {parsed.textAfter && (
        <div className="rounded-xl px-4 py-2.5 glass">
          <MarkdownViewer content={parsed.textAfter} />
        </div>
      )}
    </div>
  );
}

function EpicPreviewCard({ epic, index }: { epic: ParsedEpic; index: number }) {
  const [expanded, setExpanded] = useState(true);
  const isExisting = !!epic.id;
  const title =
    epic.name?.trim() ||
    (isExisting ? 'Existing epic' : `Epic ${index + 1}`);

  return (
    <div className="border border-purple-500/30 rounded-lg bg-purple-500/5">
      <div
        role="button"
        tabIndex={0}
        onClick={() => setExpanded(!expanded)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            setExpanded(!expanded);
          }
        }}
        className="w-full flex items-center gap-2 p-4 text-left cursor-pointer hover:bg-purple-500/10 rounded-t-lg transition-colors"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className={`h-4 w-4 text-board-text-muted transition-transform shrink-0 ${expanded ? 'rotate-90' : ''}`}
          viewBox="0 0 20 20"
          fill="currentColor"
          aria-hidden
        >
          <path fillRule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clipRule="evenodd" />
        </svg>
        {isExisting ? (
          <span className="px-2 py-0.5 rounded text-xs font-medium bg-purple-500/10 text-purple-300 border border-purple-500/20">
            Adding to epic
          </span>
        ) : (
          <span className="px-2 py-0.5 rounded text-xs font-medium bg-purple-500/20 text-purple-400">
            Epic {index + 1}
          </span>
        )}
        <h4 className="font-medium text-board-text flex-1 min-w-0">{title}</h4>
        <span className="text-xs text-board-text-muted shrink-0">
          {epic.tickets.length} ticket{epic.tickets.length !== 1 ? 's' : ''}
        </span>
      </div>

      {epic.description && expanded && !isExisting && (
        <div className="px-4 pb-2 text-sm text-board-text-muted">
          <MarkdownViewer content={epic.description} />
        </div>
      )}

      {expanded && (
        <div className="px-4 pb-4 space-y-2">
          {epic.tickets.map((ticket, i) => (
            <TicketPreviewCard key={i} ticket={ticket} />
          ))}
        </div>
      )}
    </div>
  );
}

function TicketUpdatePreviewCard({ update }: { update: ParsedUpdate }) {
  const [expanded, setExpanded] = useState(false);
  const changedFields = [
    update.title && 'title',
    update.description && 'description',
    update.priority && 'priority',
    update.tasks && 'tasks',
  ].filter(Boolean);

  return (
    <div className="border border-amber-500/30 rounded-lg p-4 bg-amber-500/5">
      <div className="flex items-center gap-2 mb-2">
        <span className="px-2 py-0.5 rounded text-xs font-medium bg-amber-500/20 text-amber-400">
          update
        </span>
        <h4 className="font-medium text-board-text">
          {update.title || `Ticket ${update.ticket_id.slice(0, 8)}...`}
        </h4>
      </div>

      <div className="text-xs text-board-text-muted">
        Changing: {changedFields.join(', ')}
      </div>

      {update.description && (
        <div className="text-sm text-board-text-muted mt-1">
          <button
            onClick={() => setExpanded(!expanded)}
            className="text-xs text-board-accent hover:underline"
          >
            {expanded ? 'Hide description' : 'Show description'}
          </button>
          {expanded && (
            <div className="mt-2">
              <MarkdownViewer content={update.description} />
            </div>
          )}
        </div>
      )}

      {update.tasks && update.tasks.length > 0 && (
        <div className="text-xs text-board-text-muted mt-2 flex items-center gap-1">
          <svg xmlns="http://www.w3.org/2000/svg" className="h-3.5 w-3.5" viewBox="0 0 20 20" fill="currentColor">
            <path d="M9 2a1 1 0 000 2h2a1 1 0 100-2H9z" />
            <path fillRule="evenodd" d="M4 5a2 2 0 012-2 3 3 0 003 3h2a3 3 0 003-3 2 2 0 012 2v11a2 2 0 01-2 2H6a2 2 0 01-2-2V5zm3 4a1 1 0 000 2h.01a1 1 0 100-2H7zm3 0a1 1 0 000 2h3a1 1 0 100-2h-3zm-3 4a1 1 0 100 2h.01a1 1 0 100-2H7zm3 0a1 1 0 100 2h3a1 1 0 100-2h-3z" clipRule="evenodd" />
          </svg>
          <span>Replacing with {update.tasks.length} task{update.tasks.length !== 1 ? 's' : ''}: {update.tasks.map(t => t.title).join(', ')}</span>
        </div>
      )}
    </div>
  );
}

function TicketPreviewCard({ ticket }: { ticket: ParsedTicket }) {
  const [expanded, setExpanded] = useState(false);
  const priority = ticket.priority || 'medium';
  const colorClass = PRIORITY_COLORS[priority] || PRIORITY_COLORS.medium;
  const isExistingLink = !!ticket.id;

  return (
    <div className="border border-board-border rounded-lg p-4 bg-board-card/30">
      <div className="flex items-center gap-2 mb-2 flex-wrap">
        {isExistingLink ? (
          <span className="px-2 py-0.5 rounded text-xs font-medium bg-blue-500/15 text-blue-300 border border-blue-500/25">
            existing ticket
          </span>
        ) : (
          <span className={`px-2 py-0.5 rounded text-xs font-medium ${colorClass}`}>
            {priority}
          </span>
        )}
        <h4 className="font-medium text-board-text">
          {ticket.title.trim() || `Ticket ${ticket.id?.slice(0, 8) ?? ''}…`}
        </h4>
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

export function parseTicketBuilderJsonFromText(text: string) {
  return tryParseJson(text);
}
