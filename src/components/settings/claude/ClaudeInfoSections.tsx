interface ClaudeInfoSectionsProps {
  configVisible: boolean;
  setConfigVisible: (visible: boolean) => void;
  configJson: string;
}

export function ClaudeInfoSections({
  configVisible,
  setConfigVisible,
  configJson,
}: ClaudeInfoSectionsProps) {
  return (
    <>
      {/* Settings File Locations */}
      <div className="glass rounded-lg p-3 space-y-2">
        <h3 className="text-sm font-medium text-board-text">
          Settings File Locations
        </h3>
        <ul className="text-xs text-board-text-muted space-y-1">
          <li>
            <span className="text-board-text-secondary">User:</span>
            <code className="ml-1 bg-board-bg px-1 rounded text-board-text-secondary">
              ~/.claude/settings.json
            </code>
          </li>
          <li>
            <span className="text-board-text-secondary">Project:</span>
            <code className="ml-1 bg-board-bg px-1 rounded text-board-text-secondary">
              .claude/settings.json
            </code>
          </li>
          <li>
            <span className="text-board-text-secondary">Local:</span>
            <code className="ml-1 bg-board-bg px-1 rounded text-board-text-secondary">
              .claude/settings.local.json
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
    "UserPromptSubmit": [...],
    "PreToolUse": [...],
    "PostToolUse": [...],
    "Stop": [...]
  }
}`}
          </pre>
        </details>
      </div>

      {/* Hook Behavior */}
      <div className="bg-status-info/10 border border-status-info/30 rounded-lg px-3 py-2">
        <h3 className="text-sm font-medium text-status-info">Hook Behavior</h3>
        <ul className="text-xs text-board-text-secondary mt-1 space-y-0.5">
          <li>
            <strong>Exit 0:</strong> Continue normally
          </li>
          <li>
            <strong>Exit 2:</strong> Blocking error, stderr to Claude
          </li>
          <li>
            <strong>UserPromptSubmit:</strong> stdout injected as context
          </li>
        </ul>
      </div>

      {/* Supported Hooks Table */}
      <div className="glass rounded-lg p-3 space-y-2">
        <h3 className="text-sm font-medium text-board-text">Supported Hooks</h3>
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="text-left text-board-text-muted border-b border-board-border">
                <th className="pb-1.5">Hook</th>
                <th className="pb-1.5">Trigger</th>
                <th className="pb-1.5">Block?</th>
              </tr>
            </thead>
            <tbody className="text-board-text-secondary">
              <tr className="border-b border-board-border/50">
                <td className="py-1.5">
                  <code className="bg-board-bg px-1 rounded">
                    UserPromptSubmit
                  </code>
                </td>
                <td className="py-1.5">User submits</td>
                <td className="py-1.5">Yes</td>
              </tr>
              <tr className="border-b border-board-border/50">
                <td className="py-1.5">
                  <code className="bg-board-bg px-1 rounded">PreToolUse</code>
                </td>
                <td className="py-1.5">Before tool</td>
                <td className="py-1.5">Yes</td>
              </tr>
              <tr className="border-b border-board-border/50">
                <td className="py-1.5">
                  <code className="bg-board-bg px-1 rounded">PostToolUse</code>
                </td>
                <td className="py-1.5">After tool</td>
                <td className="py-1.5">No</td>
              </tr>
              <tr className="border-b border-board-border/50">
                <td className="py-1.5">
                  <code className="bg-board-bg px-1 rounded">
                    PostToolUseFailure
                  </code>
                </td>
                <td className="py-1.5">Tool failed</td>
                <td className="py-1.5">No</td>
              </tr>
              <tr>
                <td className="py-1.5">
                  <code className="bg-board-bg px-1 rounded">Stop</code>
                </td>
                <td className="py-1.5">Session ends</td>
                <td className="py-1.5">Yes</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </>
  );
}
