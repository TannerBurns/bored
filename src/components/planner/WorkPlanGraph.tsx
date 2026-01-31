import type { ProjectPlan, PlanEpic } from '../../types';

interface WorkPlanGraphProps {
  plan: ProjectPlan;
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
 * Visualizes the work plan as a dependency graph.
 * Shows epics and their dependencies in a visual flow.
 */
export function WorkPlanGraph({ plan }: WorkPlanGraphProps) {
  // Build dependency map for layout
  const epicMap = new Map<string, PlanEpic>();
  plan.epics.forEach(epic => epicMap.set(epic.title, epic));
  
  // Group epics by dependency level (0 = no dependencies, 1 = depends on level 0, etc.)
  const levels = new Map<number, PlanEpic[]>();
  const epicLevels = new Map<string, number>();
  
  const getLevel = (epic: PlanEpic): number => {
    const deps = normalizeDependencies(epic.dependsOn);
    if (deps.length === 0) return 0;
    
    const cached = epicLevels.get(epic.title);
    if (cached !== undefined) return cached;
    
    // For multiple dependencies, level = max(level of all deps) + 1
    let maxParentLevel = -1;
    for (const depTitle of deps) {
      const parent = epicMap.get(depTitle);
      if (parent) {
        const parentLevel = getLevel(parent);
        maxParentLevel = Math.max(maxParentLevel, parentLevel);
      }
    }
    
    const level = maxParentLevel + 1;
    epicLevels.set(epic.title, level);
    return level;
  };
  
  // Assign all epics to levels
  plan.epics.forEach(epic => {
    const level = getLevel(epic);
    epicLevels.set(epic.title, level);
    
    if (!levels.has(level)) {
      levels.set(level, []);
    }
    levels.get(level)!.push(epic);
  });
  
  const maxLevel = Math.max(...levels.keys());
  
  return (
    <div className="space-y-8 p-6">
      {/* Overview */}
      <div className="bg-gray-50 dark:bg-gray-800 rounded-lg p-4">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-2">
          Project Overview
        </h3>
        <p className="text-gray-600 dark:text-gray-300">
          {plan.overview}
        </p>
      </div>

      {/* Dependency Graph */}
      <div className="space-y-6">
        {Array.from({ length: maxLevel + 1 }, (_, level) => (
          <div key={level} className="relative">
            {/* Level Header */}
            <div className="flex items-center gap-2 mb-3">
              <div className="bg-purple-100 dark:bg-purple-900/30 px-3 py-1 rounded-full">
                <span className="text-sm font-medium text-purple-700 dark:text-purple-300">
                  {level === 0 ? 'Starting Epics' : `Phase ${level + 1}`}
                </span>
              </div>
              {level > 0 && (
                <div className="flex-1 border-t border-dashed border-gray-300 dark:border-gray-600" />
              )}
            </div>
            
            {/* Epics in this level */}
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {(levels.get(level) || []).map((epic, idx) => (
                <EpicCard key={idx} epic={epic} />
              ))}
            </div>
            
            {/* Arrow to next level */}
            {level < maxLevel && (
              <div className="flex justify-center py-2">
                <svg className="w-6 h-8 text-gray-400" fill="none" viewBox="0 0 24 32">
                  <path 
                    d="M12 4v20m0 0l-6-6m6 6l6-6" 
                    stroke="currentColor" 
                    strokeWidth="2" 
                    strokeLinecap="round" 
                    strokeLinejoin="round"
                  />
                </svg>
              </div>
            )}
          </div>
        ))}
      </div>

      {/* Summary Stats */}
      <div className="flex gap-4 pt-4 border-t dark:border-gray-700">
        <div className="bg-blue-50 dark:bg-blue-900/20 px-4 py-2 rounded-lg">
          <span className="text-2xl font-bold text-blue-600 dark:text-blue-400">
            {plan.epics.length}
          </span>
          <span className="text-sm text-gray-600 dark:text-gray-400 ml-2">
            Epics
          </span>
        </div>
        <div className="bg-green-50 dark:bg-green-900/20 px-4 py-2 rounded-lg">
          <span className="text-2xl font-bold text-green-600 dark:text-green-400">
            {plan.epics.reduce((sum, e) => sum + e.tickets.length, 0)}
          </span>
          <span className="text-sm text-gray-600 dark:text-gray-400 ml-2">
            Tickets
          </span>
        </div>
        <div className="bg-purple-50 dark:bg-purple-900/20 px-4 py-2 rounded-lg">
          <span className="text-2xl font-bold text-purple-600 dark:text-purple-400">
            {maxLevel + 1}
          </span>
          <span className="text-sm text-gray-600 dark:text-gray-400 ml-2">
            Phases
          </span>
        </div>
      </div>
    </div>
  );
}

function EpicCard({ epic }: { epic: PlanEpic }) {
  const deps = normalizeDependencies(epic.dependsOn);
  
  return (
    <div className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg p-4 shadow-sm hover:shadow-md transition-shadow">
      <div className="flex items-start justify-between">
        <h4 className="font-medium text-gray-900 dark:text-white">
          {epic.title}
        </h4>
        <span className="bg-gray-100 dark:bg-gray-800 text-xs text-gray-600 dark:text-gray-400 px-2 py-0.5 rounded-full">
          {epic.tickets.length} ticket{epic.tickets.length !== 1 ? 's' : ''}
        </span>
      </div>
      
      <p className="text-sm text-gray-500 dark:text-gray-400 mt-2 line-clamp-2">
        {epic.description}
      </p>
      
      {deps.length > 0 && (
        <div className="mt-3 flex items-start gap-1 text-xs text-orange-600 dark:text-orange-400">
          <svg className="w-3 h-3 mt-0.5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <span>
            Depends on: {deps.length === 1 ? deps[0] : deps.join(', ')}
          </span>
        </div>
      )}
      
      {/* Ticket preview */}
      <div className="mt-3 space-y-1">
        {epic.tickets.slice(0, 3).map((ticket, idx) => (
          <div key={idx} className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
            <span className="w-1.5 h-1.5 bg-gray-300 dark:bg-gray-600 rounded-full" />
            <span className="truncate">{ticket.title}</span>
          </div>
        ))}
        {epic.tickets.length > 3 && (
          <div className="text-xs text-gray-400 dark:text-gray-500 pl-3">
            +{epic.tickets.length - 3} more
          </div>
        )}
      </div>
    </div>
  );
}
