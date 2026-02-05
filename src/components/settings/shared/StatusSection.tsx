interface StatusSectionProps {
  isAvailable: boolean;
  version?: string;
}

export function StatusSection({
  isAvailable,
  version,
}: StatusSectionProps) {
  return (
    <div className="glass rounded-lg p-3 space-y-2">
      <h3 className="text-sm font-medium text-board-text">Status</h3>

      <div className="flex gap-4 text-xs">
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
      </div>
    </div>
  );
}
