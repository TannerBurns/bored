export function ClaudeInfoSections() {
  return (
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
  );
}
