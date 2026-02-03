interface HookScriptSectionProps {
  hookPath: string | undefined;
  onCopy: () => Promise<void>;
  description?: string;
}

export function HookScriptSection({
  hookPath,
  onCopy,
  description = 'Intercepts agent lifecycle events.',
}: HookScriptSectionProps) {
  return (
    <div className="glass rounded-lg p-3 space-y-2">
      <h3 className="text-sm font-medium text-board-text">Hook Script</h3>
      <p className="text-xs text-board-text-muted">{description}</p>

      <div className="flex items-center gap-2">
        <input
          type="text"
          value={hookPath || 'Not available'}
          readOnly
          className="flex-1 px-2 py-1.5 bg-board-surface-raised rounded-lg text-xs font-mono text-board-text-secondary border border-board-border"
        />
        <button
          onClick={onCopy}
          disabled={!hookPath}
          className="px-2 py-1.5 text-xs bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors disabled:opacity-50 text-board-text"
        >
          Copy
        </button>
      </div>
    </div>
  );
}
