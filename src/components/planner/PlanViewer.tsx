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
        <div>
          <h3 className="text-lg font-semibold mb-3 text-gray-900 dark:text-white">
            Work Plan Overview
          </h3>
          <div className="prose dark:prose-invert max-w-none bg-gray-50 dark:bg-gray-800 rounded-lg p-4">
            <MarkdownViewer content={planJson.overview} />
          </div>
        </div>

        {/* Execution Flow Section */}
        <div>
          <h3 className="text-lg font-semibold mb-3 text-gray-900 dark:text-white">
            Execution Flow
          </h3>
          <div className="bg-gray-50 dark:bg-gray-800 rounded-lg p-4">
            {(() => {
              const phases = calculateExecutionPhases(planJson.epics);
              const rootCount = phases[0]?.epics.length ?? 0;
              
              return (
                <div className="space-y-4">
                  {/* Summary */}
                  <div className="text-sm text-gray-600 dark:text-gray-300">
                    {rootCount === 1 ? (
                      <span className="text-green-600 dark:text-green-400">
                        ✓ Sequential execution: 1 root epic, {phases.length} phases total
                      </span>
                    ) : rootCount === planJson.epics.length ? (
                      <span className="text-amber-600 dark:text-amber-400">
                        ⚠ All {rootCount} epics are root (no dependencies) - all can run in parallel
                      </span>
                    ) : (
                      <span>
                        {rootCount} root epic{rootCount !== 1 ? 's' : ''} (can start immediately), {phases.length} phases total
                      </span>
                    )}
                  </div>

                  {/* Phase visualization */}
                  <div className="space-y-3">
                    {phases.map(({ phase, epics: phaseEpics }) => (
                      <div key={phase} className="flex items-start gap-3">
                        <div className="flex-shrink-0 w-20 text-right">
                          <span className="inline-block bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 text-xs font-medium px-2 py-1 rounded">
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
                                  className="group relative bg-white dark:bg-gray-900 border dark:border-gray-700 rounded px-3 py-1.5 text-sm"
                                >
                                  <span className="font-medium text-gray-900 dark:text-white">
                                    {epic.title}
                                  </span>
                                  {deps.length > 0 && (
                                    <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 hidden group-hover:block z-10">
                                      <div className="bg-gray-900 dark:bg-gray-700 text-white text-xs rounded px-2 py-1 whitespace-nowrap">
                                        Depends on: {deps.join(', ')}
                                      </div>
                                    </div>
                                  )}
                                </div>
                              );
                            })}
                            {phaseEpics.length > 1 && (
                              <span className="text-xs text-gray-400 self-center">
                                (parallel)
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
          <h3 className="text-lg font-semibold mb-4 text-gray-900 dark:text-white">
            Epics ({planJson.epics.length})
          </h3>
          
          <div className="space-y-4">
            {planJson.epics.map((epic, epicIdx) => (
              <div
                key={epicIdx}
                className="border dark:border-gray-700 rounded-lg overflow-hidden"
              >
                <div className="bg-purple-50 dark:bg-purple-900/20 px-4 py-3">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="bg-purple-500 text-white text-xs font-medium px-2 py-0.5 rounded">
                      Epic {epicIdx + 1}
                    </span>
                    {(() => {
                      const deps = normalizeDependencies(epic.dependsOn);
                      if (deps.length === 0) return null;
                      return (
                        <span className="text-xs text-gray-500 dark:text-gray-400">
                          → depends on: {deps.length === 1 ? deps[0] : deps.join(', ')}
                        </span>
                      );
                    })()}
                  </div>
                  <h4 className="font-medium text-gray-900 dark:text-white mt-2">
                    {epic.title}
                  </h4>
                  <p className="text-sm text-gray-600 dark:text-gray-300 mt-1">
                    {epic.description}
                  </p>
                </div>

                <div className="divide-y dark:divide-gray-700">
                  {epic.tickets.map((ticket, ticketIdx) => (
                    <div key={ticketIdx} className="px-4 py-3 bg-white dark:bg-gray-900">
                      <div className="flex items-start gap-3">
                        <span className="text-gray-400 text-sm font-mono">
                          {epicIdx + 1}.{ticketIdx + 1}
                        </span>
                        <div className="flex-1">
                          <h5 className="font-medium text-gray-900 dark:text-white">
                            {ticket.title}
                          </h5>
                          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                            {ticket.description}
                          </p>
                          {ticket.acceptanceCriteria && ticket.acceptanceCriteria.length > 0 && (
                            <ul className="mt-2 text-sm text-gray-600 dark:text-gray-300 list-disc list-inside">
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
    <div className="prose dark:prose-invert max-w-none">
      <MarkdownViewer content={markdown} />
    </div>
  );
}
