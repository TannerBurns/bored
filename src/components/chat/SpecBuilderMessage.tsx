import { useState } from 'react';
import { parseAssistantMessage } from '../planner/parseAssistantMessage';
import { MarkdownViewer } from '../common/MarkdownViewer';

interface SpecBuilderMessageProps {
  content: string;
}

export function SpecBuilderMessage({ content }: SpecBuilderMessageProps) {
  const parsed = parseAssistantMessage(content);

  if (!parsed.hasStructure) {
    return <MarkdownViewer content={content} />;
  }

  return (
    <div className="space-y-3">
      {parsed.preamble && (
        <div className="text-sm text-board-text-muted">
          <MarkdownViewer content={parsed.preamble} />
        </div>
      )}

      {parsed.observations && (
        <CollapsibleSection
          title="Observations"
          defaultExpanded={false}
          accentColor="blue"
        >
          <MarkdownViewer content={parsed.observations} />
        </CollapsibleSection>
      )}

      {parsed.questions && (
        <CollapsibleSection
          title="Questions"
          defaultExpanded={true}
          accentColor="purple"
        >
          <MarkdownViewer content={parsed.questions} />
        </CollapsibleSection>
      )}
    </div>
  );
}

function CollapsibleSection({
  title,
  children,
  defaultExpanded = false,
  accentColor = 'purple',
}: {
  title: string;
  children: React.ReactNode;
  defaultExpanded?: boolean;
  accentColor?: 'purple' | 'blue';
}) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);

  const borderClass =
    accentColor === 'blue'
      ? 'border-blue-500/30 bg-blue-500/5'
      : 'border-purple-500/30 bg-purple-500/5';

  const headerClass =
    accentColor === 'blue' ? 'text-blue-400' : 'text-purple-400';

  return (
    <div className={`rounded-lg border ${borderClass} overflow-hidden`}>
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium hover:bg-white/5 transition-colors"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className={`h-3 w-3 transition-transform ${isExpanded ? 'rotate-90' : ''} ${headerClass}`}
          viewBox="0 0 20 20"
          fill="currentColor"
        >
          <path
            fillRule="evenodd"
            d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z"
            clipRule="evenodd"
          />
        </svg>
        <span className={headerClass}>{title}</span>
      </button>
      {isExpanded && (
        <div className="px-3 pb-3 text-sm">{children}</div>
      )}
    </div>
  );
}
