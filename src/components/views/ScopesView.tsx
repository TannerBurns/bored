import { ScopesList } from '../settings/ScopesList';

interface ScopesViewProps {
  onProjectsChange?: () => void;
}

export function ScopesView({ onProjectsChange }: ScopesViewProps) {
  return (
    <div className="flex-1 overflow-hidden flex flex-col">
      <div className="flex-1 overflow-auto glass rounded-lg p-4">
        <ScopesList onProjectsChange={onProjectsChange} />
      </div>
    </div>
  );
}
