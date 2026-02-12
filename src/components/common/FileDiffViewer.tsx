import { useState } from 'react';
import type { FileDiff, DiffLine } from '../../types';
import { cn } from '../../lib/utils';

interface FileDiffViewerProps {
  files: FileDiff[];
  className?: string;
}

export function FileDiffViewer({ files, className }: FileDiffViewerProps) {
  if (files.length === 0) {
    return (
      <div className={cn('text-xs text-board-text-muted p-3', className)}>
        No changes
      </div>
    );
  }

  return (
    <div className={cn('overflow-auto', className)}>
      {files.map((file) => (
        <FileDiffSection key={file.path} file={file} />
      ))}
    </div>
  );
}

function FileDiffSection({ file }: { file: FileDiff }) {
  const [open, setOpen] = useState(true);
  const statusColor =
    file.status === 'added'
      ? 'text-emerald-400'
      : file.status === 'deleted'
        ? 'text-red-400'
        : 'text-board-text-secondary';

  return (
    <div className="border-b border-board-border last:border-b-0">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-board-hover/50 transition-colors"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className={cn('shrink-0 transition-transform', !open && '-rotate-90')}
        >
          <polyline points="6 9 12 15 18 9" />
        </svg>
        <span className="text-xs font-mono text-board-text truncate flex-1">
          {file.path}
        </span>
        <span className={cn('text-xs font-medium', statusColor)}>
          {file.status}
        </span>
        <span className="text-xs text-board-text-muted">
          +{file.additions} / -{file.deletions}
        </span>
      </button>
      {open && (
        <div className="bg-board-bg/30 font-mono text-xs">
          {file.hunks.map((hunk, hunkIdx) => (
            <div key={`${file.path}-${hunkIdx}`}>
              <div className="px-3 py-1 bg-board-hover/50 text-board-text-muted border-y border-board-border/50">
                {hunk.header}
              </div>
              {hunk.lines.map((line, lineIdx) => (
                <DiffLineRow
                  key={`${file.path}-${hunkIdx}-${lineIdx}`}
                  line={line}
                />
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function DiffLineRow({ line }: { line: DiffLine }) {
  const isAdd = line.lineType === 'add';
  const isDelete = line.lineType === 'delete';
  const bg = isAdd
    ? 'bg-emerald-500/10'
    : isDelete
      ? 'bg-red-500/10'
      : 'bg-transparent';
  const border = isAdd
    ? 'border-l-2 border-emerald-500/50'
    : isDelete
      ? 'border-l-2 border-red-500/50'
      : '';

  return (
    <div
      className={cn(
        'flex min-w-0 px-3 py-0.5 border-l-2 border-transparent',
        bg,
        border
      )}
    >
      <div className="w-14 shrink-0 flex gap-1 text-board-text-muted select-none">
        {line.oldLineNum != null ? (
          <span className="w-6 text-right">{line.oldLineNum}</span>
        ) : (
          <span className="w-6" />
        )}
        {line.newLineNum != null ? (
          <span className="w-6 text-right">{line.newLineNum}</span>
        ) : (
          <span className="w-6" />
        )}
      </div>
      <span
        className={cn(
          'shrink-0 w-4',
          isAdd && 'text-emerald-400',
          isDelete && 'text-red-400'
        )}
      >
        {line.lineType === 'add' ? '+' : line.lineType === 'delete' ? '-' : ' '}
      </span>
      <span className="text-board-text-secondary whitespace-pre truncate">
        {line.content}
      </span>
    </div>
  );
}
