import { useState, useEffect, useRef, useMemo, memo } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useWorkerStatus } from '../../hooks/useWorkerStatus';
import { useAgentRegistryStore } from '../../stores/agentRegistryStore';
import { getAgentIcon, getAgentBrandColor, getAgentDisplayName } from '../common/AgentIcons';
import type { WorkerStatus, AgentInfo } from '../../types';

const IS_MAC = navigator.platform.startsWith('Mac');

interface TitleBarProps {
  activeNav: string;
  currentBoardName?: string;
  selectedTicketTitle?: string;
}

export const TitleBar = memo(function TitleBar({
  activeNav,
  currentBoardName,
  selectedTicketTitle,
}: TitleBarProps) {
  const { workers, queueStatus, startWorker, stopWorkerByType } = useWorkerStatus();
  const agents = useAgentRegistryStore((s) => s.agents);
  const loadAgents = useAgentRegistryStore((s) => s.loadAgents);
  const [dropdownOpen, setDropdownOpen] = useState(false);

  useEffect(() => {
    loadAgents();
  }, [loadAgents]);

  const totalWorkers = workers.length;
  const hasActive = totalWorkers > 0;

  const contextLabel = useMemo(() => {
    if (selectedTicketTitle) {
      const boardPart = currentBoardName ?? 'Board';
      return { prefix: boardPart, current: selectedTicketTitle };
    }
    switch (activeNav) {
      case 'dashboard': return { current: 'Dashboard' };
      case 'boards': return { current: currentBoardName ?? 'Boards' };
      case 'chat': return { current: 'Chat' };
      case 'specs': return { current: 'AI Specs' };
      case 'agents': return { current: 'Agents' };
      case 'projects': return { current: 'Projects' };
      case 'settings': return { current: 'Settings' };
      default: return { current: 'Bored' };
    }
  }, [activeNav, currentBoardName, selectedTicketTitle]);

  return (
    <div
      data-tauri-drag-region
      className="titlebar h-[38px] flex items-center justify-between px-4 bg-board-bg-solid border-b border-board-border/50 select-none flex-shrink-0"
    >
      {/* Left: context breadcrumb */}
      <div
        data-tauri-drag-region
        className="flex items-center gap-1.5 text-xs min-w-0"
        style={IS_MAC ? { paddingLeft: 70 } : undefined}
      >
        <span data-tauri-drag-region className="text-board-text-muted font-medium">Bored</span>
        <span data-tauri-drag-region className="text-board-text-muted/30">·</span>
        {contextLabel.prefix && (
          <>
            <span data-tauri-drag-region className="text-board-text-muted truncate max-w-[120px]">
              {contextLabel.prefix}
            </span>
            <span data-tauri-drag-region className="text-board-text-muted/30">/</span>
          </>
        )}
        <span data-tauri-drag-region className="text-board-text-secondary truncate max-w-[200px]">
          {contextLabel.current}
        </span>
      </div>

      {/* Right: workers dropdown + status pills + window controls */}
      <div className="flex items-center gap-1.5">
        <WorkersDropdownButton
          totalWorkers={totalWorkers}
          hasActive={hasActive}
          isOpen={dropdownOpen}
          onToggle={() => setDropdownOpen((o) => !o)}
          agents={agents}
          workers={workers}
          onAdd={startWorker}
          onRemove={stopWorkerByType}
          onClose={() => setDropdownOpen(false)}
        />

        {queueStatus.readyCount > 0 && (
          <StatusPill color="blue" count={queueStatus.readyCount} label="queued" />
        )}
        {queueStatus.inProgressCount > 0 && (
          <StatusPill color="amber" count={queueStatus.inProgressCount} label="active" pulse />
        )}

        {!IS_MAC && <WindowControls />}
      </div>
    </div>
  );
});

function StatusPill({
  color,
  count,
  label,
  pulse,
}: {
  color: 'blue' | 'amber';
  count: number;
  label: string;
  pulse?: boolean;
}) {
  const styles = {
    blue: {
      bg: 'bg-blue-500/15 border-blue-500/25',
      text: 'text-blue-400',
      dot: 'bg-blue-500',
    },
    amber: {
      bg: 'bg-amber-500/15 border-amber-500/25',
      text: 'text-amber-400',
      dot: 'bg-amber-500',
    },
  } as const;
  const s = styles[color];

  return (
    <span className={`flex items-center gap-1.5 px-2 py-1 rounded-full text-[11px] font-medium border ${s.bg} ${s.text}`}>
      <span className={`w-1.5 h-1.5 rounded-full ${s.dot} ${pulse ? 'animate-pulse' : ''}`} />
      {count} {label}
    </span>
  );
}

function WorkersDropdownButton({
  totalWorkers,
  hasActive,
  isOpen,
  onToggle,
  agents,
  workers,
  onAdd,
  onRemove,
  onClose,
}: {
  totalWorkers: number;
  hasActive: boolean;
  isOpen: boolean;
  onToggle: () => void;
  agents: AgentInfo[];
  workers: WorkerStatus[];
  onAdd: (agentType: string) => Promise<void>;
  onRemove: (agentType: string) => Promise<void>;
  onClose: () => void;
}) {
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('mousedown', handleClick);
    document.addEventListener('keydown', handleKey);
    return () => {
      document.removeEventListener('mousedown', handleClick);
      document.removeEventListener('keydown', handleKey);
    };
  }, [isOpen, onClose]);

  const sortedAgents = useMemo(
    () => [...agents].filter((a) => a.isAvailable).sort((a, b) => a.displayName.localeCompare(b.displayName)),
    [agents],
  );

  const workerCountByType = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const w of workers) {
      counts[w.agentType] = (counts[w.agentType] || 0) + 1;
    }
    return counts;
  }, [workers]);

  return (
    <div className="relative" ref={dropdownRef}>
      <button
        onClick={onToggle}
        className={`flex items-center gap-1.5 px-2 py-1 rounded-full text-[11px] font-medium border transition-colors ${
          hasActive
            ? 'bg-emerald-500/10 border-emerald-500/20 text-emerald-400 hover:bg-emerald-500/20'
            : 'bg-white/5 border-white/10 text-board-text-muted hover:bg-white/10'
        }`}
      >
        {hasActive && (
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" />
        )}
        <span>Workers ({totalWorkers})</span>
        <svg
          className={`w-3 h-3 transition-transform ${isOpen ? 'rotate-180' : ''}`}
          viewBox="0 0 12 12"
          fill="none"
        >
          <path d="M3 5l3 3 3-3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>

      {isOpen && (
        <div className="absolute right-0 top-full mt-1.5 z-50 w-56 rounded-lg border border-board-border bg-board-bg-solid shadow-2xl overflow-hidden">
          <div className="py-1">
            {sortedAgents.length === 0 ? (
              <div className="px-3 py-3 text-xs text-board-text-muted text-center">
                No agents available
              </div>
            ) : (
              sortedAgents.map((agent) => (
                <AgentWorkerRow
                  key={agent.id}
                  agent={agent}
                  count={workerCountByType[agent.id] ?? 0}
                  onAdd={() => onAdd(agent.id)}
                  onRemove={() => onRemove(agent.id)}
                />
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function AgentWorkerRow({
  agent,
  count,
  onAdd,
  onRemove,
}: {
  agent: AgentInfo;
  count: number;
  onAdd: () => void;
  onRemove: () => void;
}) {
  const Icon = getAgentIcon(agent.id);
  const brandColor = getAgentBrandColor(agent.id, agent.brandColor);

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 hover:bg-board-card-hover/50 transition-colors">
      {brandColor
        ? <Icon size={14} style={{ color: brandColor }} />
        : <Icon size={14} className="text-board-text-secondary" />
      }
      <span className="text-xs font-medium text-board-text flex-1 truncate">
        {getAgentDisplayName(agent.id, agent.displayName)}
      </span>
      <span className="text-xs text-board-text-muted w-4 text-center tabular-nums">
        {count}
      </span>
      <div className="flex items-center gap-0.5">
        <button
          onClick={onRemove}
          disabled={count === 0}
          className="w-5 h-5 flex items-center justify-center rounded text-board-text-muted hover:text-board-text hover:bg-board-card-hover disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
            <path d="M2.5 5h5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </button>
        <button
          onClick={onAdd}
          className="w-5 h-5 flex items-center justify-center rounded text-board-text-muted hover:text-board-text hover:bg-board-card-hover transition-colors"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
            <path d="M5 2.5v5M2.5 5h5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </button>
      </div>
    </div>
  );
}

function WindowControls() {
  const win = getCurrentWindow();

  return (
    <div className="flex items-center ml-2">
      <button
        onClick={() => win.minimize()}
        className="w-8 h-[38px] flex items-center justify-center text-board-text-muted hover:text-board-text hover:bg-white/5 transition-colors"
      >
        <svg width="10" height="1" viewBox="0 0 10 1">
          <rect width="10" height="1" fill="currentColor" />
        </svg>
      </button>
      <button
        onClick={() => win.toggleMaximize()}
        className="w-8 h-[38px] flex items-center justify-center text-board-text-muted hover:text-board-text hover:bg-white/5 transition-colors"
      >
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
          <rect x="0.5" y="0.5" width="9" height="9" stroke="currentColor" strokeWidth="1" />
        </svg>
      </button>
      <button
        onClick={() => win.close()}
        className="w-8 h-[38px] flex items-center justify-center text-board-text-muted hover:text-board-text hover:bg-red-500/80 hover:text-white transition-colors"
      >
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
          <path d="M1 1l8 8M9 1l-8 8" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
        </svg>
      </button>
    </div>
  );
}
