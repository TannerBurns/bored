import { MarkdownViewer } from '../common/MarkdownViewer';
import type { ProjectPlan, PlanEpic } from '../../types';

interface PlanViewerProps {
  markdown: string;
  planJson?: ProjectPlan;
}

/**
 * Normalize dependsOn to always be an array of strings.
 * Handles old format (string | null) and new format (string[]).
 */
function normalizeDependencies(dependsOn: string[] | string | null | undefined): string[] {
  if (!dependsOn) return [];
  if (Array.isArray(dependsOn)) return dependsOn.filter(d => d && d.length > 0);
  return [dependsOn];
}

/**
 * Calculate execution phases based on dependencies.
 * Returns an array of phases, where each phase contains epics that can run in parallel.
 */
function calculateExecutionPhases(epics: PlanEpic[]): { phase: number; epics: PlanEpic[] }[] {
  const titleToEpic = new Map<string, PlanEpic>();
  epics.forEach(e => titleToEpic.set(e.title, e));

  // Calculate level for each epic
  const levels = new Map<string, number>();
  
  function getLevel(epic: PlanEpic): number {
    if (levels.has(epic.title)) return levels.get(epic.title)!;
    
    const deps = normalizeDependencies(epic.dependsOn);
    if (deps.length === 0) {
      levels.set(epic.title, 0);
      return 0;
    }
    
    let maxDepLevel = 0;
    for (const depTitle of deps) {
      const depEpic = titleToEpic.get(depTitle);
      if (depEpic) {
        maxDepLevel = Math.max(maxDepLevel, getLevel(depEpic) + 1);
      }
    }
    levels.set(epic.title, maxDepLevel);
    return maxDepLevel;
  }

  epics.forEach(e => getLevel(e));

  // Group by level
  const phaseMap = new Map<number, PlanEpic[]>();
  epics.forEach(e => {
    const level = levels.get(e.title) ?? 0;
    if (!phaseMap.has(level)) phaseMap.set(level, []);
    phaseMap.get(level)!.push(e);
  });

  // Convert to sorted array
  const phases = Array.from(phaseMap.entries())
    .sort((a, b) => a[0] - b[0])
    .map(([phase, phaseEpics]) => ({ phase: phase + 1, epics: phaseEpics }));

  return phases;
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
            {(() => {
              const phases = calculateExecutionPhases(planJson.epics);
              const rootCount = phases[0]?.epics.length ?? 0;
              
              return (
                <div className="space-y-4">
                  {/* Summary */}
                  <div className="text-sm">
                    {rootCount === 1 ? (
                      <span className="text-status-success flex items-center gap-2">
                        <span className="w-2 h-2 rounded-full bg-status-success" />
                        Sequential execution: 1 root epic, {phases.length} phases total
                      </span>
                    ) : rootCount === planJson.epics.length ? (
                      <span className="text-status-warning flex items-center gap-2">
                        <span className="w-2 h-2 rounded-full bg-status-warning" />
                        All {rootCount} epics are root (no dependencies) - all can run in parallel
                      </span>
                    ) : (
                      <span className="text-board-text-secondary flex items-center gap-2">
                        <span className="w-2 h-2 rounded-full bg-status-info" />
                        {rootCount} root epic{rootCount !== 1 ? 's' : ''} (can start immediately), {phases.length} phases total
                      </span>
                    )}
                  </div>

                  {/* Phase visualization */}
                  <div className="space-y-3">
                    {phases.map(({ phase, epics: phaseEpics }) => (
                      <div key={phase} className="flex items-start gap-3">
                        <div className="flex-shrink-0 w-20 text-right">
                          <span className="inline-block accent-gradient text-white text-xs font-medium px-2.5 py-1 rounded-full shadow-sm">
                            Phase {phase}
                          </span>
                        </div>
                        <div className="flex-1">
                          <div className="flex flex-wrap gap-2">
                            {phaseEpics.map((epic, idx) => {
                              const deps = normalizeDependencies(epic.dependsOn);
                              return (
                                <div
                                  key={idx}
                                  className="group relative glass-intense rounded-lg px-3 py-1.5 text-sm hover:shadow-md transition-all duration-200"
                                >
                                  <span className="font-medium text-board-text">
                                    {epic.title}
                                  </span>
                                  {deps.length > 0 && (
                                    <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 hidden group-hover:block z-10">
                                      <div className="glass-intense text-board-text text-xs rounded-lg px-2 py-1 whitespace-nowrap shadow-lg">
                                        Depends on: {deps.join(', ')}
                                      </div>
                                    </div>
                                  )}
                                </div>
                              );
                            })}
                            {phaseEpics.length > 1 && (
                              <span className="text-xs text-board-text-muted self-center glass-subtle px-2 py-0.5 rounded-full">
                                parallel
                              </span>
                            )}
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              );
            })()}
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
                    <span className="accent-gradient text-white text-xs font-medium px-2.5 py-0.5 rounded-full shadow-sm">
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
