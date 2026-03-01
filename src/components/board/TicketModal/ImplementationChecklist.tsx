import { useState } from 'react';
import { cn } from '../../../lib/utils';
import type { ImplementationTodoStatus } from './types';

interface ImplementationChecklistProps {
  todos: ImplementationTodoStatus[];
}

function StatusIcon({ status }: { status: ImplementationTodoStatus['status'] }) {
  switch (status) {
    case 'completed':
      return (
        <svg className="w-4 h-4 text-status-success flex-shrink-0" viewBox="0 0 16 16" fill="none">
          <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" fill="currentColor" fillOpacity="0.15" />
          <path d="M5 8l2 2 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    case 'in_progress':
      return (
        <svg className="w-4 h-4 text-status-warning flex-shrink-0 animate-spin" viewBox="0 0 16 16" fill="none">
          <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" strokeDasharray="12 32" />
        </svg>
      );
    case 'failed':
      return (
        <svg className="w-4 h-4 text-status-error flex-shrink-0" viewBox="0 0 16 16" fill="none">
          <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" fill="currentColor" fillOpacity="0.15" />
          <path d="M6 6l4 4M10 6l-4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      );
    default:
      return (
        <svg className="w-4 h-4 text-board-text-muted flex-shrink-0" viewBox="0 0 16 16" fill="none">
          <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      );
  }
}

export function ImplementationChecklist({ todos }: ImplementationChecklistProps) {
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);

  if (todos.length === 0) return null;

  const completed = todos.filter(t => t.status === 'completed').length;
  const total = todos.length;

  return (
    <div className="mt-3">
      <div className="flex items-center justify-between mb-2">
        <p className="text-xs font-medium text-board-text-muted">
          Implementation ({completed}/{total})
        </p>
        <div className="flex-1 mx-3 h-1 bg-board-surface-raised rounded-full overflow-hidden">
          <div
            className="h-full bg-status-success rounded-full transition-all duration-300"
            style={{ width: `${total > 0 ? (completed / total) * 100 : 0}%` }}
          />
        </div>
      </div>
      <div className="space-y-0.5">
        {todos.map((todo, idx) => (
          <div key={idx}>
            <button
              onClick={() => setExpandedIndex(expandedIndex === idx ? null : idx)}
              className={cn(
                'w-full flex items-center gap-2 py-1.5 px-2 rounded text-left transition-colors',
                'hover:bg-board-surface-raised',
                expandedIndex === idx && 'bg-board-surface-raised',
              )}
            >
              <StatusIcon status={todo.status} />
              <span className={cn(
                'text-xs flex-1',
                todo.status === 'completed' ? 'text-board-text-muted' : 'text-board-text-secondary',
                todo.status === 'in_progress' && 'font-medium text-board-text-primary',
              )}>
                {todo.title}
              </span>
              <svg
                className={cn(
                  'w-3 h-3 text-board-text-muted transition-transform flex-shrink-0',
                  expandedIndex === idx && 'rotate-90',
                )}
                viewBox="0 0 12 12"
                fill="none"
              >
                <path d="M4 2l4 4-4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
            {expandedIndex === idx && (
              <div className="ml-6 mr-2 mb-1 px-2 py-1.5 text-xs text-board-text-muted whitespace-pre-wrap bg-board-surface rounded">
                {todo.description}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
