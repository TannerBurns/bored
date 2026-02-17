import { cn } from '../../lib/utils';
import { CursorIcon } from '../common';
import {
  AlertMessages,
  StatusSection,
} from './shared';
import { useCursorSettings, CursorInfoSections, type CursorCliOptionsState } from './cursor';

function CursorCliOptionsSection({ cliOptions }: { cliOptions: CursorCliOptionsState }) {
  return (
    <div className="glass rounded-lg p-3 space-y-3">
      <div>
        <h3 className="text-sm font-medium text-board-text">CLI Options</h3>
        <p className="text-xs text-board-text-muted">
          Configure Cursor agent CLI options. Changes are saved automatically.
        </p>
      </div>

      <div className="flex items-center justify-between glass-subtle rounded-lg px-3 py-2">
        <div className="mr-3">
          <span className="text-sm font-medium text-board-text">Thinking</span>
          <p className="text-xs text-board-text-muted">
            Enable extended thinking for better reasoning.
          </p>
          <p className="text-xs text-amber-500/80 mt-0.5">
            Appends &quot;-thinking&quot; to the model name sent to Cursor.
          </p>
        </div>
        <button
          onClick={() => cliOptions.setThinkingEnabled(!cliOptions.thinkingEnabled)}
          disabled={cliOptions.saving}
          className={cn(
            'relative inline-flex h-5 w-9 flex-shrink-0 cursor-pointer rounded-full transition-colors duration-200 ease-in-out focus:outline-none focus:ring-1 focus:ring-board-accent',
            cliOptions.thinkingEnabled ? 'bg-board-accent' : 'glass',
            cliOptions.saving && 'opacity-50 cursor-not-allowed'
          )}
        >
          <span
            className={cn(
              'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out',
              cliOptions.thinkingEnabled ? 'translate-x-4' : 'translate-x-0.5'
            )}
            style={{ marginTop: '2px' }}
          />
        </button>
      </div>
    </div>
  );
}

export function CursorSettings() {
  const cursor = useCursorSettings();

  if (cursor.loading) {
    return (
      <div className="text-board-text-muted text-center py-8">
        Loading Cursor status...
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <h2 className="text-lg font-semibold text-board-text flex items-center gap-2">
        <CursorIcon size={20} className="text-board-text" />
        Cursor Integration
      </h2>

      <AlertMessages error={cursor.error} success={cursor.success} />

      <StatusSection
        isAvailable={cursor.status?.isAvailable ?? false}
        version={cursor.status?.version}
      />

      <CursorCliOptionsSection cliOptions={cursor.cliOptions} />

      <CursorInfoSections />
    </div>
  );
}
