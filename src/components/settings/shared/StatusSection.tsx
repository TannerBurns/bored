interface StatusSectionProps {
  isAvailable: boolean;
  version?: string;
  hooksInstalled: boolean;
  commandsInstalled: boolean;
  hooksLabel?: string;
}

export function StatusSection({
  isAvailable,
  version,
  hooksInstalled,
  commandsInstalled,
  hooksLabel = 'User hooks',
}: StatusSectionProps) {
  return (
    <div className="glass rounded-lg p-3 space-y-2">
      <h3 className="text-sm font-medium text-board-text">Status</h3>

      <div className="grid grid-cols-2 gap-2 text-xs">
        <div className="flex items-center gap-1.5">
          <span
            className={`w-1.5 h-1.5 rounded-full ${isAvailable ? 'bg-status-success' : 'bg-status-error'}`}
          />
          <span className="text-board-text-muted">CLI:</span>
          <span className="text-board-text">
            {isAvailable ? 'Available' : 'Not found'}
          </span>
        </div>

        {version && (
          <div className="flex items-center gap-1.5">
            <span className="text-board-text-muted">Version:</span>
            <span className="text-board-text">{version}</span>
          </div>
        )}

        <div className="flex items-center gap-1.5">
          <span
            className={`w-1.5 h-1.5 rounded-full ${hooksInstalled ? 'bg-status-success' : 'bg-status-warning'}`}
          />
          <span className="text-board-text-muted">{hooksLabel}:</span>
          <span className="text-board-text">
            {hooksInstalled ? 'Installed' : 'Not installed'}
          </span>
        </div>

        <div className="flex items-center gap-1.5">
          <span
            className={`w-1.5 h-1.5 rounded-full ${commandsInstalled ? 'bg-status-success' : 'bg-status-warning'}`}
          />
          <span className="text-board-text-muted">Commands:</span>
          <span className="text-board-text">
            {commandsInstalled ? 'Installed' : 'Not installed'}
          </span>
        </div>
      </div>
    </div>
  );
}
