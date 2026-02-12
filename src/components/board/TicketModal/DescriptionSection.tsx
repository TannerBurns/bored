import { useState } from 'react';
import { MarkdownViewer } from '../../common/MarkdownViewer';

export interface DescriptionSectionProps {
  description: string;
  isEditing: boolean;
  editDescription: string;
  setEditDescription: (desc: string) => void;
  onOpenFullscreen: () => void;
}

export function DescriptionSection({
  description,
  isEditing,
  editDescription,
  setEditDescription,
  onOpenFullscreen,
}: DescriptionSectionProps) {
  const [isCollapsed, setIsCollapsed] = useState(!isEditing);

  return (
    <div>
      <div className="flex items-center justify-between mb-2">
        <button
          onClick={() => setIsCollapsed((prev) => !prev)}
          className="flex items-center gap-1.5 text-base font-semibold text-board-text hover:text-board-accent transition-colors"
          aria-expanded={!isCollapsed}
        >
          Description
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className={`transition-transform duration-200 ${isCollapsed ? '' : 'rotate-90'}`}
          >
            <polyline points="9 18 15 12 9 6" />
          </svg>
        </button>
        {!isEditing && !isCollapsed && (
          <button
            onClick={onOpenFullscreen}
            className="p-1 text-board-text-muted hover:text-board-text transition-colors rounded hover:bg-board-surface"
            aria-label="Expand description"
            title="View fullscreen"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <polyline points="15 3 21 3 21 9" />
              <polyline points="9 21 3 21 3 15" />
              <line x1="21" y1="3" x2="14" y2="10" />
              <line x1="3" y1="21" x2="10" y2="14" />
            </svg>
          </button>
        )}
      </div>
      {!isCollapsed && (
        <>
          {isEditing ? (
            <textarea
              value={editDescription}
              onChange={(e) => setEditDescription(e.target.value)}
              rows={6}
              className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text text-sm resize-none focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
              placeholder="Add a description..."
            />
          ) : (
            <div className="bg-board-surface rounded-lg p-3">
              <MarkdownViewer content={description} />
            </div>
          )}
        </>
      )}
    </div>
  );
}
