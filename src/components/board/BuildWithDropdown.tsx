import { useState, useRef, useEffect } from 'react';
import { ClaudeIcon, CursorIcon } from '../common';
import { useCliAvailability } from '../../hooks/useCliAvailability';

interface BuildWithDropdownProps {
  onSelect: (agent: 'cursor' | 'claude') => void;
  disabled?: boolean;
  disabledReason?: string;
  /** Button label (default: "Build with") */
  label?: string;
  /** Tooltip shown on hover (e.g. explains what the button does) */
  title?: string;
}

export function BuildWithDropdown({
  onSelect,
  disabled = false,
  disabledReason,
  label = 'Build with',
  title: titleProp,
}: BuildWithDropdownProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [openUpward, setOpenUpward] = useState(false);
  const { cursorAvailable, claudeAvailable } = useCliAvailability();
  const dropdownRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const handleToggle = () => {
    if (disabled) return;
    
    if (!isOpen && buttonRef.current) {
      const buttonRect = buttonRef.current.getBoundingClientRect();
      const dropdownHeight = 100; // Approximate height of dropdown menu
      const spaceBelow = window.innerHeight - buttonRect.bottom;
      const spaceAbove = buttonRect.top;
      
      // Open upward if not enough space below but enough above
      setOpenUpward(spaceBelow < dropdownHeight && spaceAbove > dropdownHeight);
    }
    
    setIsOpen(!isOpen);
  };

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    }

    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [isOpen]);

  useEffect(() => {
    function handleEscape(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        setIsOpen(false);
      }
    }

    if (isOpen) {
      document.addEventListener('keydown', handleEscape);
      return () => document.removeEventListener('keydown', handleEscape);
    }
  }, [isOpen]);

  const handleSelect = (agent: 'cursor' | 'claude') => {
    setIsOpen(false);
    onSelect(agent);
  };

  return (
    <div ref={dropdownRef} className="relative">
      <button
        ref={buttonRef}
        onClick={handleToggle}
        disabled={disabled}
        title={titleProp ?? (disabled && disabledReason ? disabledReason : undefined)}
        className={`
          glass rounded-xl px-4 py-2 text-sm font-medium
          flex items-center gap-2
          transition-all duration-200
          ${disabled
            ? 'opacity-50 cursor-not-allowed'
            : 'hover:glass-intense hover:shadow-md hover:border-board-accent/50 cursor-pointer'
          }
          text-board-text
        `}
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
          className="text-board-accent"
        >
          <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
        </svg>
        <span>{label}</span>
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
          className={`transition-transform duration-200 ${isOpen ? 'rotate-180' : ''}`}
        >
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>

      {isOpen && (
        <div className={`absolute left-0 z-50 glass-intense rounded-xl shadow-lg py-1 min-w-[160px] border border-board-border overflow-hidden ${
          openUpward ? 'bottom-full mb-1' : 'top-full mt-1'
        }`}>
          <button
            onClick={() => cursorAvailable && handleSelect('cursor')}
            disabled={!cursorAvailable}
            title={!cursorAvailable ? 'Cursor CLI not available' : undefined}
            className={`w-full text-left px-3 py-2.5 text-sm transition-colors flex items-center gap-3 ${
              cursorAvailable
                ? 'text-board-text hover:bg-board-card-hover cursor-pointer'
                : 'text-board-text-muted cursor-not-allowed opacity-50'
            }`}
          >
            <CursorIcon className={cursorAvailable ? 'text-board-text-secondary' : 'text-board-text-muted'} />
            <span>Cursor</span>
            {!cursorAvailable && <span className="text-xs text-board-text-muted ml-auto">(not installed)</span>}
          </button>
          <button
            onClick={() => claudeAvailable && handleSelect('claude')}
            disabled={!claudeAvailable}
            title={!claudeAvailable ? 'Claude CLI not available' : undefined}
            className={`w-full text-left px-3 py-2.5 text-sm transition-colors flex items-center gap-3 ${
              claudeAvailable
                ? 'text-board-text hover:bg-board-card-hover cursor-pointer'
                : 'text-board-text-muted cursor-not-allowed opacity-50'
            }`}
          >
            <ClaudeIcon className={claudeAvailable ? 'text-[#da7756]' : 'text-board-text-muted'} />
            <span>Claude</span>
            {!claudeAvailable && <span className="text-xs text-board-text-muted ml-auto">(not installed)</span>}
          </button>
        </div>
      )}
    </div>
  );
}
