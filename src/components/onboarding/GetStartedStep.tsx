import { cn } from '../../lib/utils';

interface GetStartedStepProps {
  onComplete: () => void;
  onBack: () => void;
}

export function GetStartedStep({ onComplete, onBack }: GetStartedStepProps) {
  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="text-center space-y-3">
        <div className="w-16 h-16 mx-auto bg-status-success/20 rounded-2xl flex items-center justify-center">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="32"
            height="32"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="text-status-success"
          >
            <polyline points="20 6 9 17 4 12" />
          </svg>
        </div>
        <h2 className="text-xl font-semibold text-board-text">You're All Set!</h2>
        <p className="text-board-text-secondary max-w-md mx-auto">
          Here's what you can do next to start building with AI agents.
        </p>
      </div>

      {/* Feature cards */}
      <div className="space-y-3">
        {/* Tickets */}
        <div className="p-4 bg-board-surface-raised border border-board-border rounded-lg">
          <div className="flex items-start gap-3">
            <div className="w-10 h-10 bg-board-accent/20 rounded-lg flex items-center justify-center flex-shrink-0">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
                className="text-board-accent"
              >
                <rect width="8" height="4" x="8" y="2" rx="1" ry="1" />
                <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
              </svg>
            </div>
            <div>
              <div className="text-sm font-medium text-board-text">Create Tickets</div>
              <p className="text-xs text-board-text-muted mt-0.5">
                Add coding tasks for AI agents to complete. Click "New" in the board view to create a ticket.
              </p>
            </div>
          </div>
        </div>

        {/* Specs */}
        <div className="p-4 bg-board-surface-raised border border-board-border rounded-lg">
          <div className="flex items-start gap-3">
            <div className="w-10 h-10 bg-purple-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
                className="text-purple-400"
              >
                <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
                <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
              </svg>
            </div>
            <div>
              <div className="text-sm font-medium text-board-text">Use AI Specs</div>
              <p className="text-xs text-board-text-muted mt-0.5">
                For complex features, use the Specs view to brainstorm and plan with AI before breaking into tickets.
              </p>
            </div>
          </div>
        </div>

        {/* Agents */}
        <div className="p-4 bg-board-surface-raised border border-board-border rounded-lg">
          <div className="flex items-start gap-3">
            <div className="w-10 h-10 bg-status-success/20 rounded-lg flex items-center justify-center flex-shrink-0">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
                className="text-status-success"
              >
                <path d="M12 8V4H8" />
                <rect width="16" height="12" x="4" y="8" rx="2" />
                <path d="M2 14h2" />
                <path d="M20 14h2" />
                <path d="M15 13v2" />
                <path d="M9 13v2" />
              </svg>
            </div>
            <div>
              <div className="text-sm font-medium text-board-text">Run Agents</div>
              <p className="text-xs text-board-text-muted mt-0.5">
                Click "Build with" on any ticket to run an agent. Or start Workers from the Agents view for hands-off automation.
              </p>
            </div>
          </div>
        </div>
      </div>

      {/* Navigation */}
      <div className="flex justify-between pt-4 border-t border-board-border">
        <button
          onClick={onBack}
          className="px-4 py-2 text-board-text-muted hover:text-board-text transition-colors"
        >
          Back
        </button>
        <button
          onClick={onComplete}
          className={cn(
            'px-6 py-2.5 bg-board-accent text-white rounded-lg transition-colors',
            'hover:bg-board-accent-hover'
          )}
        >
          Get Started
        </button>
      </div>
    </div>
  );
}
