import { useState, useEffect } from 'react';
import { SpecList, SpecDetail } from '../planner';
import { useSpecStore } from '../../stores/specStore';
import type { Board, SpecWithVersion } from '../../types';

interface SpecsViewProps {
  currentBoard: Board | null;
  onCreateSpecClick: () => void;
  onOpenChat?: (specId: string) => void;
}

function SidebarToggle({ collapsed, onClick }: { collapsed: boolean; onClick: () => void }) {
  return (
    <div className="flex-shrink-0 flex items-start pt-3">
      <button
        onClick={onClick}
        className="w-5 h-8 rounded-md glass border border-board-border/40 flex items-center justify-center text-board-text-muted hover:text-board-text hover:border-board-border transition-colors"
        title={collapsed ? 'Show specs list' : 'Hide specs list'}
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
          className={`transition-transform duration-200 ${collapsed ? 'rotate-180' : ''}`}
        >
          <polyline points="15 18 9 12 15 6" />
        </svg>
      </button>
    </div>
  );
}

export function SpecsView({ currentBoard, onCreateSpecClick, onOpenChat }: SpecsViewProps) {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [selectedSpec, setSelectedSpec] = useState<SpecWithVersion | null>(null);
  
  const selectSpec = useSpecStore((s) => s.selectSpec);
  const selectSpecForProgress = useSpecStore((s) => s.selectSpecForProgress);
  const currentSpec = useSpecStore((s) => s.currentSpec);

  useEffect(() => {
    setSelectedSpec(currentSpec);
  }, [currentSpec]);

  const handleSelectSpec = (spec: SpecWithVersion | null) => {
    selectSpec(spec);
    setSelectedSpec(spec);
  };

  const handleViewProgress = (spec: SpecWithVersion) => {
    selectSpecForProgress(spec);
    setSelectedSpec(spec);
  };

  return (
    <div className="flex-1 overflow-hidden flex gap-1">
      {!sidebarCollapsed && (
        <div className="w-80 flex-shrink-0 glass rounded-2xl overflow-hidden flex flex-col">
          <div className="flex items-center justify-between px-4 py-3 border-b border-board-border">
            <h2 className="text-sm font-semibold text-board-text">Specs</h2>
            <button
              onClick={onCreateSpecClick}
              disabled={!currentBoard}
              className="flex items-center gap-1 px-2.5 py-1 text-xs font-medium rounded-lg bg-board-accent text-white hover:bg-board-accent-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              title={currentBoard ? 'Create new spec' : 'Select a board first'}
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <line x1="12" y1="5" x2="12" y2="19" />
                <line x1="5" y1="12" x2="19" y2="12" />
              </svg>
              New
            </button>
          </div>
          <div className="flex-1 overflow-y-auto">
            <SpecList onSelect={handleSelectSpec} onViewProgress={handleViewProgress} />
          </div>
        </div>
      )}

      <SidebarToggle
        collapsed={sidebarCollapsed}
        onClick={() => setSidebarCollapsed((c) => !c)}
      />

      <div className="flex-1 glass rounded-2xl overflow-hidden min-w-0">
        {selectedSpec ? (
          <SpecDetail
            spec={selectedSpec}
            onClose={() => handleSelectSpec(null)}
            onOpenChat={onOpenChat}
          />
        ) : (
          <div className="flex items-center justify-center h-full text-board-text-muted">
            <div className="text-center glass-subtle rounded-xl p-8">
              <svg
                className="w-12 h-12 mx-auto mb-3 opacity-50"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                <polyline points="14 2 14 8 20 8" />
                <line x1="16" y1="13" x2="8" y2="13" />
                <line x1="16" y1="17" x2="8" y2="17" />
                <polyline points="10 9 9 9 8 9" />
              </svg>
              <p>Select a spec to view details</p>
              <p className="text-sm mt-1">or create a new one to start planning</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
