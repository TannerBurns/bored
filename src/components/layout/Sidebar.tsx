import { useState, useRef, useEffect } from 'react';
import { cn } from '../../lib/utils';
import type { Board } from '../../types';

interface NavItem {
  id: string;
  label: string;
  icon?: React.ReactNode;
}

interface SidebarProps {
  navItems: NavItem[];
  activeItem: string;
  onItemClick: (id: string) => void;
  boards: Board[];
  currentBoard: Board | null;
  onBoardSelect: (boardId: string) => void;
  onCreateBoard: () => void;
  onRenameBoard: (board: Board) => void;
  onDeleteBoard: (board: Board) => void;
}

export function Sidebar({
  navItems,
  activeItem,
  onItemClick,
  boards,
  currentBoard,
  onBoardSelect,
  onCreateBoard,
  onRenameBoard,
  onDeleteBoard,
}: SidebarProps) {
  const [menuOpenForBoard, setMenuOpenForBoard] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  // Close menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setMenuOpenForBoard(null);
      }
    };

    if (menuOpenForBoard) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [menuOpenForBoard]);
  return (
    <aside className="w-64 bg-board-column border-r border-board-border p-4 flex flex-col">
      <h1 className="text-xl font-bold text-board-text mb-6 flex items-center gap-2">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="24"
          height="24"
          viewBox="0 0 512 512"
          className="flex-shrink-0"
        >
          <rect x="51" y="51" width="410" height="410" rx="82" className="fill-board-accent"/>
          <path d="M185 135 h95 c58 0 95 32 95 79 c0 36 -22 58 -52 69 c42 11 69 42 69 84 c0 52 -42 90 -112 90 h-95 z M235 278 h38 c32 0 48 -16 48 -42 c0 -25 -16 -42 -48 -42 h-38 z M235 395 h43 c36 0 57 -19 57 -50 c0 -32 -21 -52 -57 -52 h-43 z" fill="white"/>
        </svg>
        Bored
      </h1>

      {/* Boards Section */}
      <div className="mb-4">
        <div className="flex items-center justify-between mb-2">
          <span className="text-xs font-semibold text-board-text-muted uppercase tracking-wider">
            Boards
          </span>
          <button
            onClick={onCreateBoard}
            className="p-1 text-board-text-muted hover:text-board-text hover:bg-board-card-hover rounded transition-colors"
            aria-label="Create new board"
            title="Create new board"
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
              <line x1="12" y1="5" x2="12" y2="19" />
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
          </button>
        </div>
        <ul className="space-y-1">
          {boards.length === 0 ? (
            <li className="text-sm text-board-text-muted px-3 py-2">
              No boards yet
            </li>
          ) : (
            boards.map((board) => {
              const isActive = currentBoard?.id === board.id && activeItem === 'boards';
              const isMenuOpen = menuOpenForBoard === board.id;
              
              return (
                <li key={board.id} className="relative group">
                  <div className="flex items-center">
                    <button
                      onClick={() => {
                        onBoardSelect(board.id);
                        onItemClick('boards');
                      }}
                      className={cn(
                        'flex-1 text-left px-3 py-2 rounded-lg transition-all duration-150',
                        'flex items-center gap-2 text-sm',
                        isActive
                          ? 'bg-board-accent text-white shadow-sm'
                          : 'text-board-text-secondary hover:bg-board-card-hover hover:text-board-text'
                      )}
                    >
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
                        className="flex-shrink-0"
                      >
                        <rect x="3" y="3" width="7" height="7" />
                        <rect x="14" y="3" width="7" height="7" />
                        <rect x="3" y="14" width="7" height="7" />
                        <rect x="14" y="14" width="7" height="7" />
                      </svg>
                      <span className="truncate">{board.name}</span>
                    </button>
                    
                    {/* Three-dot menu button */}
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setMenuOpenForBoard(isMenuOpen ? null : board.id);
                      }}
                      className={cn(
                        'p-1 rounded transition-colors',
                        isActive
                          ? 'text-white/70 hover:text-white hover:bg-white/10'
                          : 'text-board-text-muted hover:text-board-text hover:bg-board-card-hover',
                        'opacity-0 group-hover:opacity-100',
                        isMenuOpen && 'opacity-100'
                      )}
                      aria-label="Board options"
                    >
                      <svg
                        xmlns="http://www.w3.org/2000/svg"
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                      >
                        <circle cx="12" cy="5" r="2" />
                        <circle cx="12" cy="12" r="2" />
                        <circle cx="12" cy="19" r="2" />
                      </svg>
                    </button>
                  </div>
                  
                  {/* Dropdown menu */}
                  {isMenuOpen && (
                    <div
                      ref={menuRef}
                      className="absolute right-0 top-full mt-1 z-50 bg-board-column border border-board-border rounded-lg shadow-lg py-1 min-w-[120px]"
                    >
                      <button
                        onClick={() => {
                          setMenuOpenForBoard(null);
                          onRenameBoard(board);
                        }}
                        className="w-full text-left px-3 py-2 text-sm text-board-text hover:bg-board-card-hover transition-colors flex items-center gap-2"
                      >
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
                        >
                          <path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" />
                          <path d="m15 5 4 4" />
                        </svg>
                        Rename
                      </button>
                      <button
                        onClick={() => {
                          setMenuOpenForBoard(null);
                          onDeleteBoard(board);
                        }}
                        className="w-full text-left px-3 py-2 text-sm text-status-error hover:bg-board-card-hover transition-colors flex items-center gap-2"
                      >
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
                        >
                          <path d="M3 6h18" />
                          <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
                          <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
                          <line x1="10" y1="11" x2="10" y2="17" />
                          <line x1="14" y1="11" x2="14" y2="17" />
                        </svg>
                        Delete
                      </button>
                    </div>
                  )}
                </li>
              );
            })
          )}
        </ul>
      </div>

      <div className="border-t border-board-border my-2" />

      <nav className="flex-1">
        <ul className="space-y-1">
          {navItems.map((item) => (
            <li key={item.id}>
              <button
                onClick={() => onItemClick(item.id)}
                className={cn(
                  'w-full text-left px-3 py-2.5 rounded-lg transition-all duration-150',
                  'flex items-center gap-2 font-medium',
                  activeItem === item.id
                    ? 'bg-board-accent text-white shadow-sm'
                    : 'text-board-text-secondary hover:bg-board-card-hover hover:text-board-text'
                )}
              >
                {item.icon}
                {item.label}
              </button>
            </li>
          ))}
        </ul>
      </nav>
      <div className="pt-4 border-t border-board-border">
        <p className="text-xs text-board-text-muted">v0.1.0</p>
      </div>
    </aside>
  );
}
