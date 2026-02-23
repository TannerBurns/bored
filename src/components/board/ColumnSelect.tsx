import { useState, useRef, useEffect } from 'react';
import { cn } from '../../lib/utils';
import { getColumnColors } from '../../lib/constants';
import type { Column } from '../../types';

interface ColumnSelectProps {
  columns: Column[];
  currentColumnId: string;
  onMove: (newColumnId: string) => void;
  size?: 'sm' | 'md';
}

export function ColumnSelect({ columns, currentColumnId, onMove, size = 'sm' }: ColumnSelectProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const current = columns.find((c) => c.id === currentColumnId);
  const currentColors = getColumnColors(current?.name ?? '');
  const sorted = [...columns].sort((a, b) => a.position - b.position);

  useEffect(() => {
    if (!open) return;
    const handleClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [open]);

  const isMd = size === 'md';

  return (
    <div ref={ref} className="relative" onClick={(e) => e.stopPropagation()}>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className={cn(
          'flex items-center gap-2 rounded-lg font-medium transition-all duration-150 cursor-pointer',
          'bg-board-surface-raised hover:bg-board-card-hover border border-board-border',
          isMd ? 'px-3 py-1.5 text-sm' : 'px-2.5 py-1 text-xs',
        )}
      >
        <span className={cn('rounded-full flex-shrink-0', currentColors.dot, isMd ? 'w-2.5 h-2.5' : 'w-2 h-2')} />
        <span className="text-board-text truncate">{current?.name ?? 'Unknown'}</span>
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width={isMd ? 14 : 12}
          height={isMd ? 14 : 12}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className={cn('text-board-text-muted transition-transform duration-150', open && 'rotate-180')}
        >
          <polyline points="6 9 12 15 18 9" />
        </svg>
      </button>

      {open && (
        <div className="absolute z-50 mt-1 min-w-[160px] py-1 rounded-xl border border-board-border shadow-lg bg-board-popover">
          {sorted.map((col) => {
            const colors = getColumnColors(col.name);
            const isActive = col.id === currentColumnId;
            return (
              <button
                key={col.id}
                type="button"
                onClick={() => {
                  if (col.id !== currentColumnId) {
                    onMove(col.id);
                  }
                  setOpen(false);
                }}
                className={cn(
                  'w-full flex items-center gap-2.5 text-left transition-colors duration-100',
                  isMd ? 'px-3.5 py-2 text-sm' : 'px-3 py-1.5 text-xs',
                  isActive
                    ? 'bg-board-accent/15 font-semibold text-board-text'
                    : 'text-board-text-secondary hover:bg-board-popover-hover',
                )}
              >
                <span className={cn('rounded-full flex-shrink-0', colors.dot, isMd ? 'w-3 h-3' : 'w-2.5 h-2.5')} />
                <span className="truncate">{col.name}</span>
                {isActive && (
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width={isMd ? 14 : 12}
                    height={isMd ? 14 : 12}
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="3"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    className="ml-auto text-board-accent flex-shrink-0"
                  >
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                )}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
