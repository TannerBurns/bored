import { useState, useMemo } from 'react';
import { MarkdownViewer } from '../common/MarkdownViewer';

interface PlanBuilderMessageProps {
  content: string;
}

interface PlanTicket {
  title: string;
  description: string;
  acceptanceCriteria?: string[];
  branchName?: string;
}

interface PlanEpic {
  title: string;
  description: string;
  dependsOn: string[];
  tickets: PlanTicket[];
}

interface ProjectPlan {
  overview: string;
  epics: PlanEpic[];
}

export function looksLikePlanResponse(content: string): boolean {
  return content.includes('"epics"') && content.includes('"overview"');
}

function cleanPreamble(raw: string): string {
  return raw.replace(/```\w*\s*$/, '').trim();
}

function extractPlanJson(content: string): { plan: ProjectPlan; preamble: string } | null {
  const jsonBlockMatch = content.match(/```json\s*([\s\S]*?)```/);
  if (jsonBlockMatch) {
    try {
      const plan = JSON.parse(jsonBlockMatch[1]) as ProjectPlan;
      if (plan?.epics && Array.isArray(plan.epics)) {
        const preamble = cleanPreamble(content.slice(0, content.indexOf(jsonBlockMatch[0])));
        return { plan, preamble };
      }
    } catch { /* fall through */ }
  }

  const braceStart = content.indexOf('{');
  const braceEnd = content.lastIndexOf('}');
  if (braceStart !== -1 && braceEnd > braceStart) {
    const candidate = content.slice(braceStart, braceEnd + 1);
    if (candidate.includes('"epics"')) {
      try {
        const plan = JSON.parse(candidate) as ProjectPlan;
        if (plan?.epics && Array.isArray(plan.epics)) {
          const preamble = cleanPreamble(content.slice(0, braceStart));
          return { plan, preamble };
        }
      } catch { /* fall through */ }
    }
  }

  return null;
}

export function PlanBuilderMessage({ content }: PlanBuilderMessageProps) {
  const parsed = useMemo(() => extractPlanJson(content), [content]);

  if (!parsed) {
    return <MarkdownViewer content={content} />;
  }

  const { plan, preamble } = parsed;
  const totalTickets = plan.epics.reduce((sum, e) => sum + e.tickets.length, 0);

  return (
    <div className="space-y-3">
      {preamble && (
        <div className="text-sm text-board-text-muted">
          <MarkdownViewer content={preamble} />
        </div>
      )}

      <div className="rounded-lg border border-emerald-500/30 bg-emerald-500/5 p-3">
        <div className="flex items-center gap-2 mb-2">
          <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4 text-emerald-400" viewBox="0 0 20 20" fill="currentColor">
            <path fillRule="evenodd" d="M6 2a2 2 0 00-2 2v12a2 2 0 002 2h8a2 2 0 002-2V7.414A2 2 0 0015.414 6L12 2.586A2 2 0 0010.586 2H6zm2 10a1 1 0 10-2 0v3a1 1 0 102 0v-3zm2-3a1 1 0 011 1v5a1 1 0 11-2 0v-5a1 1 0 011-1zm4-1a1 1 0 10-2 0v7a1 1 0 102 0V8z" clipRule="evenodd" />
          </svg>
          <span className="text-xs font-medium text-emerald-400">
            Work Plan — {plan.epics.length} epic{plan.epics.length !== 1 ? 's' : ''}, {totalTickets} ticket{totalTickets !== 1 ? 's' : ''}
          </span>
        </div>
        <p className="text-sm text-board-text">{plan.overview}</p>
      </div>

      {plan.epics.map((epic, i) => (
        <EpicCard key={i} epic={epic} index={i} />
      ))}

      <CollapsibleSection title="Raw JSON" defaultExpanded={false} accentColor="gray">
        <pre className="text-xs overflow-x-auto whitespace-pre-wrap break-words font-mono bg-black/20 rounded p-3">
          {JSON.stringify(plan, null, 2)}
        </pre>
      </CollapsibleSection>
    </div>
  );
}

function EpicCard({ epic, index }: { epic: PlanEpic; index: number }) {
  const [isExpanded, setIsExpanded] = useState(false);

  return (
    <div className="rounded-lg border border-board-border bg-board-card/30 overflow-hidden">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-start gap-2 px-3 py-2.5 text-left hover:bg-white/5 transition-colors"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className={`h-3 w-3 mt-1 flex-shrink-0 transition-transform text-board-text-muted ${isExpanded ? 'rotate-90' : ''}`}
          viewBox="0 0 20 20"
          fill="currentColor"
        >
          <path
            fillRule="evenodd"
            d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z"
            clipRule="evenodd"
          />
        </svg>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-xs font-medium text-board-accent">Epic {index + 1}</span>
            <span className="text-sm font-medium text-board-text">{epic.title}</span>
            <span className="text-[10px] text-board-text-muted">
              {epic.tickets.length} ticket{epic.tickets.length !== 1 ? 's' : ''}
            </span>
          </div>
          {epic.dependsOn.length > 0 && (
            <div className="text-[10px] text-board-text-muted mt-0.5">
              Depends on: {epic.dependsOn.join(', ')}
            </div>
          )}
        </div>
      </button>

      {isExpanded && (
        <div className="px-3 pb-3 space-y-2 border-t border-board-border/50 pt-2">
          <p className="text-xs text-board-text-muted line-clamp-3">{epic.description}</p>
          {epic.tickets.map((ticket, j) => (
            <TicketRow key={j} ticket={ticket} epicIndex={index} ticketIndex={j} />
          ))}
        </div>
      )}
    </div>
  );
}

function TicketRow({ ticket, epicIndex, ticketIndex }: { ticket: PlanTicket; epicIndex: number; ticketIndex: number }) {
  const [showDetail, setShowDetail] = useState(false);

  return (
    <div className="rounded border border-board-border/50 bg-board-card/20">
      <button
        onClick={() => setShowDetail(!showDetail)}
        className="w-full flex items-center gap-2 px-2.5 py-1.5 text-left hover:bg-white/5 transition-colors"
      >
        <span className="text-[10px] text-board-text-muted font-mono flex-shrink-0">
          {epicIndex + 1}.{ticketIndex + 1}
        </span>
        <span className="text-xs text-board-text truncate">{ticket.title}</span>
        {ticket.branchName && (
          <span className="text-[10px] text-board-text-muted font-mono ml-auto flex-shrink-0 hidden sm:inline">
            {ticket.branchName}
          </span>
        )}
      </button>

      {showDetail && (
        <div className="px-2.5 pb-2 border-t border-board-border/30 pt-2 text-xs">
          <MarkdownViewer content={ticket.description} />
          {ticket.acceptanceCriteria && ticket.acceptanceCriteria.length > 0 && (
            <div className="mt-2">
              <span className="text-[10px] font-medium text-board-text-muted">Acceptance Criteria</span>
              <ul className="list-disc list-inside text-board-text-muted mt-0.5 space-y-0.5">
                {ticket.acceptanceCriteria.map((c, i) => (
                  <li key={i}>{c}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function CollapsibleSection({
  title,
  children,
  defaultExpanded = false,
  accentColor = 'gray',
}: {
  title: string;
  children: React.ReactNode;
  defaultExpanded?: boolean;
  accentColor?: string;
}) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);

  const borderClass =
    accentColor === 'gray'
      ? 'border-board-border/50 bg-board-card/20'
      : 'border-board-border bg-board-card/30';

  return (
    <div className={`rounded-lg border ${borderClass} overflow-hidden`}>
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium hover:bg-white/5 transition-colors text-board-text-muted"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className={`h-3 w-3 transition-transform ${isExpanded ? 'rotate-90' : ''}`}
          viewBox="0 0 20 20"
          fill="currentColor"
        >
          <path
            fillRule="evenodd"
            d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z"
            clipRule="evenodd"
          />
        </svg>
        <span>{title}</span>
      </button>
      {isExpanded && (
        <div className="px-3 pb-3">{children}</div>
      )}
    </div>
  );
}
