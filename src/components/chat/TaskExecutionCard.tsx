import { useState, memo } from 'react';
import { useTaskExecution, type TaskWithStages } from '../../hooks/useTaskExecution';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { STAGE_GROUP_ORDER, type StageGroup } from './stageLabels';

interface TaskExecutionCardProps {
  taskIds: string[];
  ticketId: string | undefined;
  /** Optional inline task metadata from the system message (titles only) */
  fallbackTitles?: string[];
}

export const TaskExecutionCard = memo(function TaskExecutionCard({
  taskIds,
  ticketId,
  fallbackTitles,
}: TaskExecutionCardProps) {
  const { tasks, isAllComplete, isLoading } = useTaskExecution(taskIds, ticketId);

  if (taskIds.length === 0 || (isLoading && tasks.length === 0)) {
    if (!fallbackTitles || fallbackTitles.length === 0) return null;
    return (
      <div className="max-w-[85%]">
        <div className="rounded-lg border border-amber-500/30 bg-amber-500/5 p-3">
          <div className="flex items-center gap-2">
            <TasksIcon />
            <span className="text-xs font-medium text-amber-400">Fix Tasks</span>
            {isLoading && (
              <span className="w-2 h-2 rounded-full bg-amber-400 animate-pulse" />
            )}
          </div>
          <div className="mt-2 space-y-1">
            {fallbackTitles.map((title, i) => (
              <div key={i} className="flex items-center gap-2 text-xs text-board-text pl-1">
                <span className="w-1.5 h-1.5 rounded-full bg-yellow-500/60 flex-shrink-0" />
                <span>{title}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    );
  }

  const allSucceeded = isAllComplete && tasks.every((t) => t.task.status === 'completed');
  const hasRunning = tasks.some((t) => t.task.status === 'in_progress');
  const variant = isAllComplete
    ? (allSucceeded ? 'success' : 'error')
    : (hasRunning ? 'active' : 'pending');

  const variantStyles = {
    success: { border: 'border-emerald-500/30', bg: 'bg-emerald-500/5', text: 'text-emerald-400' },
    error:   { border: 'border-red-500/30',     bg: 'bg-red-500/5',     text: 'text-red-400' },
    active:  { border: 'border-blue-500/30',    bg: 'bg-blue-500/5',    text: 'text-blue-400' },
    pending: { border: 'border-amber-500/30',   bg: 'bg-amber-500/5',   text: 'text-amber-400' },
  } as const;
  const { border: borderColor, bg: bgColor, text: headerColor } = variantStyles[variant];

  return (
    <div className="max-w-[85%]">
      <div className={`rounded-lg border ${borderColor} ${bgColor} overflow-hidden`}>
        <div className="flex items-center gap-2 px-3 py-2">
          <TasksIcon />
          <span className={`text-xs font-medium ${headerColor}`}>
            {isAllComplete ? 'Fix Tasks Complete' : 'Fix Tasks'}
          </span>
          {!isAllComplete && (
            <span className="text-[10px] text-board-text-muted">
              {tasks.filter((t) => t.task.status === 'completed' || t.task.status === 'failed').length}/{tasks.length} done
            </span>
          )}
        </div>

        <div className="px-3 pb-3 space-y-2">
          {tasks.map((tw) => (
            <TaskRow key={tw.task.id} taskWithStages={tw} />
          ))}
        </div>
      </div>
    </div>
  );
});

const TaskRow = memo(function TaskRow({
  taskWithStages,
}: {
  taskWithStages: TaskWithStages;
}) {
  const { task, stages, currentStage } = taskWithStages;
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="rounded-md border border-board-border/40 bg-board-card/20 overflow-hidden">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-2.5 py-2 text-left hover:bg-board-card-hover/30 transition-colors"
      >
        <TaskStatusDot status={task.status} />
        <span className="text-xs font-medium text-board-text flex-1 truncate">
          {task.title || 'Fix task'}
        </span>
        {currentStage && task.status === 'in_progress' && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/15 text-blue-400 font-medium">
            {currentStage}
          </span>
        )}
        <TaskStatusBadge status={task.status} />
        <svg
          className={`w-3 h-3 text-board-text-muted transition-transform flex-shrink-0 ${expanded ? 'rotate-90' : ''}`}
          viewBox="0 0 12 12"
          fill="none"
        >
          <path d="M4 2l4 4-4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>

      {expanded && (
        <div className="border-t border-board-border/30 px-2.5 py-2 space-y-2">
          {stages.length > 0 && <StageStepper stages={stages} />}

          {task.content && (
            <div className="text-xs text-board-text-muted">
              <MarkdownViewer content={task.content} />
            </div>
          )}
        </div>
      )}
    </div>
  );
});

function StageStepper({ stages }: { stages: StageGroup[] }) {
  const displayStages = stages.length > 0 ? stages : STAGE_GROUP_ORDER.map((label) => ({
    label,
    status: 'pending' as const,
  }));

  return (
    <div className="flex items-center gap-0.5 flex-wrap">
      {displayStages.map((stage, i) => (
        <div key={stage.label} className="flex items-center gap-0.5">
          {i > 0 && (
            <div className={`w-3 h-px ${
              stage.status === 'finished' ? 'bg-emerald-500/50' :
              stage.status === 'running' ? 'bg-blue-500/50' :
              'bg-board-border/40'
            }`} />
          )}
          <div className="flex items-center gap-1">
            <StageStatusIcon status={stage.status} />
            <span className={`text-[10px] ${
              stage.status === 'running' ? 'text-blue-400 font-medium' :
              stage.status === 'finished' ? 'text-emerald-400' :
              stage.status === 'error' ? 'text-red-400' :
              'text-board-text-muted'
            }`}>
              {stage.label}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}

function StageStatusIcon({ status }: { status: StageGroup['status'] }) {
  switch (status) {
    case 'finished':
      return (
        <svg className="w-3 h-3 text-emerald-400" viewBox="0 0 12 12" fill="none">
          <circle cx="6" cy="6" r="5" stroke="currentColor" strokeWidth="1" fill="currentColor" fillOpacity="0.15" />
          <path d="M4 6l1.5 1.5L8 5" stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    case 'running':
      return (
        <svg className="w-3 h-3 text-blue-400 animate-spin" viewBox="0 0 12 12" fill="none">
          <circle cx="6" cy="6" r="5" stroke="currentColor" strokeWidth="1" strokeDasharray="8 24" />
        </svg>
      );
    case 'error':
      return (
        <svg className="w-3 h-3 text-red-400" viewBox="0 0 12 12" fill="none">
          <circle cx="6" cy="6" r="5" stroke="currentColor" strokeWidth="1" fill="currentColor" fillOpacity="0.15" />
          <path d="M4.5 4.5l3 3M7.5 4.5l-3 3" stroke="currentColor" strokeWidth="1" strokeLinecap="round" />
        </svg>
      );
    default:
      return (
        <svg className="w-3 h-3 text-board-text-muted/40" viewBox="0 0 12 12" fill="none">
          <circle cx="6" cy="6" r="5" stroke="currentColor" strokeWidth="1" />
        </svg>
      );
  }
}

function TaskStatusDot({ status }: { status: string }) {
  const colors: Record<string, string> = {
    pending: 'bg-yellow-500/60',
    in_progress: 'bg-blue-500 animate-pulse',
    completed: 'bg-emerald-500',
    failed: 'bg-red-500',
  };
  return (
    <span className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${colors[status] || colors.pending}`} />
  );
}

function TaskStatusBadge({ status }: { status: string }) {
  const styles: Record<string, string> = {
    pending: 'bg-yellow-500/15 text-yellow-400',
    in_progress: 'bg-blue-500/15 text-blue-400',
    completed: 'bg-emerald-500/15 text-emerald-400',
    failed: 'bg-red-500/15 text-red-400',
  };
  const labels: Record<string, string> = {
    pending: 'pending',
    in_progress: 'running',
    completed: 'done',
    failed: 'failed',
  };
  return (
    <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${styles[status] || styles.pending}`}>
      {labels[status] || status}
    </span>
  );
}

function TasksIcon() {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" className="h-3.5 w-3.5 text-current" viewBox="0 0 20 20" fill="currentColor">
      <path d="M9 2a1 1 0 000 2h2a1 1 0 100-2H9z" />
      <path fillRule="evenodd" d="M4 5a2 2 0 012-2 3 3 0 003 3h2a3 3 0 003-3 2 2 0 012 2v11a2 2 0 01-2 2H6a2 2 0 01-2-2V5zm3 4a1 1 0 000 2h.01a1 1 0 100-2H7zm3 0a1 1 0 000 2h3a1 1 0 100-2h-3zm-3 4a1 1 0 100 2h.01a1 1 0 100-2H7zm3 0a1 1 0 100 2h3a1 1 0 100-2h-3z" clipRule="evenodd" />
    </svg>
  );
}
