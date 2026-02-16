import { useState, useRef, useEffect } from 'react';
import { getAgentIcon, getAgentBrandColor } from '../common/AgentIcons';
import { getAvailableAgents } from '../../lib/tauri';
import type { AgentInfo, AgentType } from '../../types';

interface BuildWithDropdownProps {
  onSelect: (agent: AgentType) => void;
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
  const [agents, setAgents] = useState<AgentInfo[] | null>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  // Fetch available agents from the registry
  useEffect(() => {
    getAvailableAgents()
      .then(setAgents)
      .catch(() => setAgents(null)); // Fall back to hardcoded if registry unavailable
  }, []);

  const handleToggle = () => {
    if (disabled) return;
    
    if (!isOpen && buttonRef.current) {
      const buttonRect = buttonRef.current.getBoundingClientRect();
      const dropdownHeight = 100;
      const spaceBelow = window.innerHeight - buttonRect.bottom;
      const spaceAbove = buttonRect.top;
      
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

  const handleSelect = (agent: AgentType) => {
    setIsOpen(false);
    onSelect(agent);
  };

  /** Get icon color for agents. Uses brand color from registry when available. */
  const getIconStyle = (agentId: string, available: boolean): { className?: string; style?: React.CSSProperties } => {
    if (!available) return { className: 'text-board-text-muted' };
    const brandColor = getAgentBrandColor(agentId, agents?.find(a => a.id === agentId)?.brandColor);
    if (brandColor) return { style: { color: brandColor } };
    return { className: 'text-board-text-secondary' };
  };

  // Build the agent list from registry; empty if registry unavailable yet
  const agentList: { id: AgentType; name: string; available: boolean }[] = agents
    ? agents.map((a) => ({ id: a.id as AgentType, name: a.displayName, available: a.isAvailable }))
    : [];

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
          {agentList.map((agent) => {
            const Icon = getAgentIcon(agent.id);
            const available = agent.available;
            return (
              <button
                key={agent.id}
                onClick={() => available && handleSelect(agent.id)}
                disabled={!available}
                title={!available ? `${agent.name} CLI not available` : undefined}
                className={`w-full text-left px-3 py-2.5 text-sm transition-colors flex items-center gap-3 ${
                  available
                    ? 'text-board-text hover:bg-board-card-hover cursor-pointer'
                    : 'text-board-text-muted cursor-not-allowed opacity-50'
                }`}
              >
                <Icon {...getIconStyle(agent.id, available)} />
                <span>{agent.name}</span>
                {!available && <span className="text-xs text-board-text-muted ml-auto">(not installed)</span>}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
