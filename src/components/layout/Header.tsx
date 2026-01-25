import { ReactNode } from 'react';

interface HeaderProps {
  title: string;
  subtitle?: string;
  action?: ReactNode;
}

export function Header({ title, subtitle, action }: HeaderProps) {
  return (
    <header className="mb-6 flex items-start justify-between">
      <div>
        <h2 className="text-2xl font-semibold text-board-text">{title}</h2>
        {subtitle && (
          <p className="text-board-text-muted mt-1">{subtitle}</p>
        )}
      </div>
      {action && <div>{action}</div>}
    </header>
  );
}
