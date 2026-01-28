import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { open } from '@tauri-apps/api/shell';
import { cn } from '../../lib/utils';

interface MarkdownViewerProps {
  content: string;
  className?: string;
}

// Custom link component that opens URLs in the system's default browser
function ExternalLink({
  href,
  children,
}: {
  href?: string;
  children?: React.ReactNode;
}) {
  const handleClick = (e: React.MouseEvent<HTMLAnchorElement>) => {
    e.preventDefault();
    if (href) {
      // Open in system's default browser using Tauri shell API
      open(href).catch((err) => {
        console.error('Failed to open link:', err);
      });
    }
  };

  return (
    <a href={href} onClick={handleClick} className="cursor-pointer">
      {children}
    </a>
  );
}

export function MarkdownViewer({ content, className }: MarkdownViewerProps) {
  if (!content) {
    return (
      <span className="text-board-text-muted italic">No description</span>
    );
  }

  return (
    <div
      className={cn(
        'prose prose-sm dark:prose-invert max-w-none',
        // Custom styling for board theme
        'prose-headings:text-board-text prose-headings:font-semibold',
        'prose-p:text-board-text-secondary prose-p:my-2',
        'prose-a:text-board-accent prose-a:no-underline hover:prose-a:underline',
        'prose-strong:text-board-text prose-strong:font-semibold',
        'prose-code:text-board-accent prose-code:bg-board-surface-raised prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:before:content-none prose-code:after:content-none',
        'prose-pre:bg-board-surface-raised prose-pre:border prose-pre:border-board-border',
        'prose-blockquote:border-l-board-accent prose-blockquote:text-board-text-muted',
        'prose-ul:text-board-text-secondary prose-ol:text-board-text-secondary',
        'prose-li:my-0.5',
        'prose-hr:border-board-border',
        'prose-table:text-board-text-secondary',
        'prose-th:text-board-text prose-th:border-board-border prose-th:px-3 prose-th:py-2',
        'prose-td:border-board-border prose-td:px-3 prose-td:py-2',
        className
      )}
    >
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ExternalLink,
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
