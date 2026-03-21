import { memo } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { open } from '@tauri-apps/plugin-shell';
import { cn } from '../../lib/utils';

interface MarkdownViewerProps {
  content: string;
  className?: string;
}

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

const REMARK_PLUGINS = [remarkGfm];

const MARKDOWN_COMPONENTS = {
  a: ExternalLink,
  table: ({ children, ...props }: React.ComponentPropsWithoutRef<'table'>) => (
    <div className="overflow-x-auto">
      <table {...props}>{children}</table>
    </div>
  ),
};

export const MarkdownViewer = memo(function MarkdownViewer({ content, className }: MarkdownViewerProps) {
  if (!content) {
    return (
      <span className="text-board-text-muted italic">No description</span>
    );
  }

  return (
    <div
      className={cn(
        'prose prose-sm dark:prose-invert max-w-none',
        'prose-headings:text-board-text prose-headings:font-semibold',
        'prose-p:text-board-text-secondary prose-p:my-2',
        'prose-a:text-board-accent prose-a:no-underline hover:prose-a:underline',
        'prose-strong:text-board-text prose-strong:font-semibold',
        'prose-code:text-board-accent prose-code:glass-subtle prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded-lg prose-code:text-sm prose-code:before:content-none prose-code:after:content-none',
        'prose-pre:glass prose-pre:rounded-xl prose-pre:border-none prose-pre:relative prose-pre:overflow-x-auto',
        'prose-blockquote:border-l-4 prose-blockquote:border-l-board-accent prose-blockquote:text-board-text-muted prose-blockquote:pl-4 prose-blockquote:italic',
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
        remarkPlugins={REMARK_PLUGINS}
        components={MARKDOWN_COMPONENTS}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
});
