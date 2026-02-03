import { useState, useEffect } from 'react';
import { SpecList, SpecDetail } from '../planner';
import { useSpecStore } from '../../stores/specStore';
import type { Board, SpecWithVersion } from '../../types';

interface SpecsViewProps {
  currentBoard: Board | null;
  onCreateSpecClick: () => void;
}

export function SpecsView({ currentBoard, onCreateSpecClick }: SpecsViewProps) {
  const [isSpecListCollapsed, setIsSpecListCollapsed] = useState(false);
  const [selectedSpec, setSelectedSpec] = useState<SpecWithVersion | null>(null);
  
  const { selectSpec, currentSpec } = useSpecStore();

  useEffect(() => {
    setSelectedSpec(currentSpec);
  }, [currentSpec]);

  const handleSelectSpec = (spec: SpecWithVersion | null) => {
    selectSpec(spec);
    setSelectedSpec(spec);
  };

  return (
    <div className="flex-1 overflow-hidden flex gap-4">
      <div className={`${isSpecListCollapsed ? 'w-12' : 'w-80'} glass rounded-2xl overflow-hidden flex flex-col transition-all duration-300`}>
        <div className={`p-4 border-b border-board-border flex items-center glass-subtle ${isSpecListCollapsed ? 'justify-center' : 'justify-between'}`}>
          {!isSpecListCollapsed && <h3 className="font-semibold text-board-text">Specs</h3>}
          <div className={`flex items-center gap-1`}>
            {!isSpecListCollapsed && (
              <button
                onClick={onCreateSpecClick}
                disabled={!currentBoard}
                className="p-1.5 text-board-text-muted hover:text-board-text hover:bg-board-card-hover rounded-lg transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
                title={currentBoard ? 'Create new spec' : 'Select a board first'}
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
            )}
            <button
              onClick={() => setIsSpecListCollapsed(!isSpecListCollapsed)}
              className="p-1.5 text-board-text-muted hover:text-board-text hover:bg-board-card-hover rounded-lg transition-all duration-200"
              title={isSpecListCollapsed ? 'Expand specs list' : 'Collapse specs list'}
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
                className={`transition-transform duration-300 ${isSpecListCollapsed ? 'rotate-180' : ''}`}
              >
                <polyline points="15 18 9 12 15 6" />
              </svg>
            </button>
          </div>
        </div>
        {!isSpecListCollapsed && (
          <div className="flex-1 overflow-y-auto">
            <SpecList onSelect={handleSelectSpec} />
          </div>
        )}
        {isSpecListCollapsed && (
          <div className="flex-1 flex flex-col items-center pt-2">
            <button
              onClick={onCreateSpecClick}
              disabled={!currentBoard}
              className="p-2 text-board-text-muted hover:text-board-text hover:bg-board-card-hover rounded-lg transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
              title={currentBoard ? 'Create new spec' : 'Select a board first'}
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
        )}
      </div>
      
      <div className="flex-1 glass rounded-2xl overflow-hidden">
        {selectedSpec ? (
          <SpecDetail
            spec={selectedSpec}
            onClose={() => handleSelectSpec(null)}
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
