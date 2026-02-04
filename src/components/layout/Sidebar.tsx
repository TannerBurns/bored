import { useState, useRef, useEffect } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { cn } from '../../lib/utils';
import { BoredLogo } from '../common/BoredLogo';
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
  onSettingsClick?: () => void;
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
  onSettingsClick,
}: SidebarProps) {
  const [menuOpenForBoard, setMenuOpenForBoard] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState<string>('');
  const menuRef = useRef<HTMLDivElement>(null);

  // Fetch app version on mount
  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => setAppVersion('unknown'));
  }, []);

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
    <aside className="w-64 glass-intense border-r border-board-border p-4 flex flex-col">
      {/* Logo with settings icon */}
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-bold text-board-text flex items-center gap-2 group">
          <div className="relative">
            <BoredLogo 
              size={28} 
              variant="gradient" 
              className="flex-shrink-0 transition-transform duration-300 group-hover:scale-110"
            />
            <div className="absolute inset-0 -z-10 blur-xl opacity-40 rounded-full bg-board-accent" />
          </div>
          <span className="bg-clip-text text-transparent bg-gradient-to-r from-board-text to-board-text-secondary">
            Bored
          </span>
        </h1>
        {/* Settings icon button */}
        {onSettingsClick && (
          <button
            onClick={onSettingsClick}
            className={cn(
              'p-2 rounded-lg transition-all duration-200',
              activeItem === 'settings'
                ? 'bg-board-accent text-white shadow-md'
                : 'text-board-text-muted hover:text-board-text hover:bg-board-card-hover'
            )}
            aria-label="Settings"
            title="Settings"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="18"
              height="18"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
          </button>
        )}
      </div>

      {/* Boards Section */}
      <div className="mb-4">
        <div className="flex items-center justify-between mb-2">
          <span className="text-xs font-semibold text-board-text-muted uppercase tracking-wider">
            Boards
          </span>
          <button
            onClick={onCreateBoard}
            className="p-1.5 text-board-text-muted hover:text-board-text rounded-lg transition-all duration-200 hover:bg-board-card-hover hover:shadow-sm"
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
            <li className="text-sm text-board-text-muted px-3 py-2 glass-subtle rounded-lg">
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
                        'flex-1 text-left px-3 py-2 rounded-lg transition-all duration-200',
                        'flex items-center gap-2 text-sm',
                        isActive
                          ? 'bg-board-accent text-white shadow-md'
                          : 'text-board-text-secondary hover:bg-board-card-hover hover:text-board-text hover:translate-x-0.5'
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
                        'p-1 rounded-lg transition-all duration-200',
                        'text-board-text-muted hover:text-board-text hover:bg-board-card-hover',
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
                  
                  {/* Dropdown menu with glass effect */}
                  {isMenuOpen && (
                    <div
                      ref={menuRef}
                      className="absolute right-0 top-full mt-1 z-50 glass-intense rounded-xl shadow-lg py-1 min-w-[140px] border border-board-border overflow-hidden"
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
                        className="w-full text-left px-3 py-2 text-sm text-status-error hover:bg-status-error/10 transition-colors flex items-center gap-2"
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

      <div className="border-t border-board-border my-2 opacity-50" />

      <nav className="flex-1">
        <ul className="space-y-1">
          {navItems.map((item) => (
            <li key={item.id}>
              <button
                onClick={() => onItemClick(item.id)}
                className={cn(
                  'w-full text-left px-3 py-2.5 rounded-lg transition-all duration-200',
                  'flex items-center gap-2 font-medium',
                  activeItem === item.id
                    ? 'bg-board-accent text-white shadow-md'
                    : 'text-board-text-secondary hover:bg-board-card-hover hover:text-board-text hover:translate-x-0.5'
                )}
              >
                {item.icon}
                {item.label}
              </button>
            </li>
          ))}
        </ul>
      </nav>
      <div className="pt-4 border-t border-board-border border-opacity-50">
        <p className="text-xs text-board-text-muted">v{appVersion || '...'}</p>
      </div>
    </aside>
  );
}
