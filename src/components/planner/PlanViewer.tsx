import { MarkdownViewer } from '../common/MarkdownViewer';
import { ExecutionGraph } from './ExecutionGraph';
import type { ProjectPlan } from '../../types';
import { normalizeDependencies } from '../../lib/utils';

interface PlanViewerProps {
  markdown: string;
  planJson?: ProjectPlan;
}

export function PlanViewer({ markdown, planJson }: PlanViewerProps) {
  // If we have structured JSON, show overview + rendered epics
  // Otherwise fall back to full markdown
  if (planJson) {
    return (
      <div className="space-y-6">
        {/* Overview Section */}
        <div className="glass rounded-xl overflow-hidden">
          <div className="p-4 border-b border-board-border glass-subtle">
            <h3 className="text-lg font-semibold text-board-text">
              Work Plan Overview
            </h3>
          </div>
          <div className="p-4">
            <MarkdownViewer content={planJson.overview} />
          </div>
        </div>

        {/* Execution Flow Section */}
        <div className="glass rounded-xl overflow-hidden">
          <div className="p-4 border-b border-board-border glass-subtle">
            <h3 className="text-lg font-semibold text-board-text">
              Execution Flow
            </h3>
          </div>
          <div className="p-4">
            <ExecutionGraph epics={planJson.epics} />
          </div>
        </div>

        {/* Epics Breakdown */}
        <div>
          <h3 className="text-lg font-semibold mb-4 text-board-text">
            Epics ({planJson.epics.length})
          </h3>
          
          <div className="space-y-4">
            {planJson.epics.map((epic, epicIdx) => (
              <div
                key={epicIdx}
                className="glass rounded-xl overflow-hidden"
              >
                <div className="p-4 glass-subtle border-b border-board-border">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="bg-board-accent text-white text-xs font-medium px-2.5 py-0.5 rounded-full shadow-sm">
                      Epic {epicIdx + 1}
                    </span>
                    {(() => {
                      const deps = normalizeDependencies(epic.dependsOn);
                      if (deps.length === 0) return null;
                      return (
                        <span className="text-xs text-board-text-muted">
                          → depends on: {deps.length === 1 ? deps[0] : deps.join(', ')}
                        </span>
                      );
                    })()}
                  </div>
                  <h4 className="font-medium text-board-text mt-2">
                    {epic.title}
                  </h4>
                  <p className="text-sm text-board-text-secondary mt-1">
                    {epic.description}
                  </p>
                </div>

                <div className="divide-y divide-board-border">
                  {epic.tickets.map((ticket, ticketIdx) => (
                    <div key={ticketIdx} className="px-4 py-3">
                      <div className="flex items-start gap-3">
                        <span className="text-board-text-muted text-sm font-mono glass-subtle px-2 py-0.5 rounded">
                          {epicIdx + 1}.{ticketIdx + 1}
                        </span>
                        <div className="flex-1">
                          <h5 className="font-medium text-board-text">
                            {ticket.title}
                          </h5>
                          {ticket.branchName && (
                            <div className="flex items-center gap-1.5 mt-1">
                              <svg className="w-3 h-3 text-board-text-muted flex-shrink-0" viewBox="0 0 16 16" fill="currentColor">
                                <path fillRule="evenodd" d="M11.75 2.5a.75.75 0 100 1.5.75.75 0 000-1.5zm-2.25.75a2.25 2.25 0 113 2.122V6A2.5 2.5 0 0110 8.5H6a1 1 0 00-1 1v1.128a2.251 2.251 0 11-1.5 0V5.372a2.25 2.25 0 111.5 0v1.836A2.492 2.492 0 016 7h4a1 1 0 001-1v-.628A2.25 2.25 0 019.5 3.25zM4.25 12a.75.75 0 100 1.5.75.75 0 000-1.5zM3.5 3.25a.75.75 0 111.5 0 .75.75 0 01-1.5 0z" />
                              </svg>
                              <code className="text-xs text-board-text-muted font-mono">
                                {ticket.branchName}
                              </code>
                            </div>
                          )}
                          <p className="text-sm text-board-text-muted mt-1">
                            {ticket.description}
                          </p>
                          {ticket.acceptanceCriteria && ticket.acceptanceCriteria.length > 0 && (
                            <ul className="mt-2 text-sm text-board-text-secondary list-disc list-inside">
                              {ticket.acceptanceCriteria.map((criteria, i) => (
                                <li key={i}>{criteria}</li>
                              ))}
                            </ul>
                          )}
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    );
  }

  // Fallback: show full markdown if no structured data
  return (
    <div className="glass rounded-xl p-6">
      <MarkdownViewer content={markdown} />
    </div>
  );
}
