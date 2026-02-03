interface CursorInfoSectionsProps {
  hookPath?: string;
  configVisible: boolean;
  setConfigVisible: (visible: boolean) => void;
  configJson: string;
}

export function CursorInfoSections({
  hookPath,
  configVisible,
  setConfigVisible,
  configJson,
}: CursorInfoSectionsProps) {
  return (
    <>
      {/* Manual Setup Section */}
      <div className="glass rounded-lg p-3 space-y-2">
        <h3 className="text-sm font-medium text-board-text">Manual Setup</h3>
        <p className="text-xs text-board-text-muted">
          If automatic installation doesn't work, manually create/edit:
        </p>
        <ul className="text-xs text-board-text-muted list-disc list-inside space-y-0.5">
          <li>
            Global:{' '}
            <code className="bg-board-surface-raised px-1 py-0.5 rounded text-board-text-secondary">
              ~/.cursor/hooks.json
            </code>
          </li>
          <li>
            Project:{' '}
            <code className="bg-board-surface-raised px-1 py-0.5 rounded text-board-text-secondary">
              .cursor/hooks.json
            </code>
          </li>
        </ul>

        <details
          className="text-xs"
          open={configVisible}
          onToggle={(e) =>
            setConfigVisible((e.target as HTMLDetailsElement).open)
          }
        >
          <summary className="cursor-pointer text-board-accent hover:text-board-accent-hover">
            View example configuration
          </summary>
          <pre className="mt-1.5 p-2 bg-board-bg rounded-lg overflow-x-auto text-xs text-board-text-secondary border border-board-border">
            {configJson ||
              `{
  "hooks": {
    "beforeShellExecution": {
      "command": "${hookPath || '/path/to/cursor-hook.js'}",
      "args": ["beforeShellExecution"]
    },
    "afterFileEdit": {
      "command": "${hookPath || '/path/to/cursor-hook.js'}",
      "args": ["afterFileEdit"]
    },
    "stop": {
      "command": "${hookPath || '/path/to/cursor-hook.js'}",
      "args": ["stop"]
    }
  }
}`}
          </pre>
        </details>
      </div>

      {/* Beta Limitations Warning */}
      <div className="bg-status-warning/10 border border-status-warning/30 rounded-lg px-3 py-2">
        <h3 className="text-sm font-medium text-status-warning">
          Beta Limitations
        </h3>
        <p className="text-xs text-board-text-secondary mt-0.5">
          Some hooks are informational only:
        </p>
        <ul className="text-xs text-board-text-secondary list-disc list-inside mt-1 space-y-0.5">
          <li>
            <code className="bg-status-warning/10 px-1 rounded text-status-warning">
              beforeSubmitPrompt
            </code>{' '}
            - can't block
          </li>
          <li>
            <code className="bg-status-warning/10 px-1 rounded text-status-warning">
              afterFileEdit
            </code>{' '}
            - can't block
          </li>
        </ul>
      </div>
    </>
  );
}
