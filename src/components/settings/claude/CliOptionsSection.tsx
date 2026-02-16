import { cn } from '../../../lib/utils';
import type { ClaudeCliOptionsState } from './useClaudeSettings';

interface CliOptionsSectionProps {
  cliOptions: ClaudeCliOptionsState;
}

interface ToggleRowProps {
  label: string;
  description: string;
  note?: string;
  enabled: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
}

function ToggleRow({ label, description, note, enabled, onChange, disabled }: ToggleRowProps) {
  return (
    <div className="flex items-center justify-between glass-subtle rounded-lg px-3 py-2">
      <div className="mr-3">
        <span className="text-sm font-medium text-board-text">{label}</span>
        <p className="text-xs text-board-text-muted">{description}</p>
        {note && (
          <p className="text-xs text-amber-500/80 mt-0.5">{note}</p>
        )}
      </div>
      <button
        onClick={() => onChange(!enabled)}
        disabled={disabled}
        className={cn(
          'relative inline-flex h-5 w-9 flex-shrink-0 cursor-pointer rounded-full transition-colors duration-200 ease-in-out focus:outline-none focus:ring-1 focus:ring-board-accent',
          enabled ? 'bg-board-accent' : 'glass',
          disabled && 'opacity-50 cursor-not-allowed'
        )}
      >
        <span
          className={cn(
            'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out',
            enabled ? 'translate-x-4' : 'translate-x-0.5'
          )}
          style={{ marginTop: '2px' }}
        />
      </button>
    </div>
  );
}

export function CliOptionsSection({ cliOptions }: CliOptionsSectionProps) {
  return (
    <div className="glass rounded-lg p-3 space-y-3">
      <div>
        <h3 className="text-sm font-medium text-board-text">CLI Options</h3>
        <p className="text-xs text-board-text-muted">
          Configure Claude Code CLI flags. Changes are saved automatically.
        </p>
      </div>

      <ToggleRow
        label="Thinking"
        description="Enable extended thinking for better reasoning."
        enabled={cliOptions.thinkingEnabled}
        onChange={cliOptions.setThinkingEnabled}
        disabled={cliOptions.saving}
      />

      <ToggleRow
        label="Extended Context (1M tokens)"
        description="Enable 1M token context window for larger codebases."
        note="Only works with API key users, not OAuth/console users."
        enabled={cliOptions.extendedContext}
        onChange={cliOptions.setExtendedContext}
        disabled={cliOptions.saving}
      />

      <ToggleRow
        label="Chrome"
        description="Enable browser automation via Chrome."
        enabled={cliOptions.chromeEnabled}
        onChange={cliOptions.setChromeEnabled}
        disabled={cliOptions.saving}
      />
    </div>
  );
}
