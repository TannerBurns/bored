export function CursorInfoSections() {
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
                  beforeShellExecution
                </code>
              </td>
              <td className="py-1.5">Before shell command</td>
              <td className="py-1.5">Yes</td>
            </tr>
            <tr className="border-b border-board-border/50">
              <td className="py-1.5">
                <code className="bg-board-bg px-1 rounded">beforeReadFile</code>
              </td>
              <td className="py-1.5">Before file read</td>
              <td className="py-1.5">Yes</td>
            </tr>
            <tr className="border-b border-board-border/50">
              <td className="py-1.5">
                <code className="bg-board-bg px-1 rounded">
                  beforeMCPExecution
                </code>
              </td>
              <td className="py-1.5">Before MCP tool</td>
              <td className="py-1.5">Yes</td>
            </tr>
            <tr className="border-b border-board-border/50">
              <td className="py-1.5">
                <code className="bg-board-bg px-1 rounded">afterFileEdit</code>
              </td>
              <td className="py-1.5">After file edit</td>
              <td className="py-1.5">No</td>
            </tr>
            <tr>
              <td className="py-1.5">
                <code className="bg-board-bg px-1 rounded">stop</code>
              </td>
              <td className="py-1.5">Session ends</td>
              <td className="py-1.5">No</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  );
}
