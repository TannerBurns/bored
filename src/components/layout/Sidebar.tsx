import { cn } from '../../lib/utils';

interface NavItem {
  id: string;
  label: string;
  icon?: React.ReactNode;
}

interface SidebarProps {
  navItems: NavItem[];
  activeItem: string;
  onItemClick: (id: string) => void;
}

export function Sidebar({ navItems, activeItem, onItemClick }: SidebarProps) {
  return (
    <aside className="w-64 bg-board-column border-r border-board-border p-4 flex flex-col">
      <h1 className="text-xl font-bold text-board-text mb-6 flex items-center gap-2">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="24"
          height="24"
          viewBox="0 0 512 512"
          className="flex-shrink-0"
        >
          <rect x="51" y="51" width="410" height="410" rx="82" className="fill-board-accent"/>
          <path d="M185 135 h95 c58 0 95 32 95 79 c0 36 -22 58 -52 69 c42 11 69 42 69 84 c0 52 -42 90 -112 90 h-95 z M235 278 h38 c32 0 48 -16 48 -42 c0 -25 -16 -42 -48 -42 h-38 z M235 395 h43 c36 0 57 -19 57 -50 c0 -32 -21 -52 -57 -52 h-43 z" fill="white"/>
        </svg>
        Bored
      </h1>
      <nav className="flex-1">
        <ul className="space-y-1">
          {navItems.map((item) => (
            <li key={item.id}>
              <button
                onClick={() => onItemClick(item.id)}
                className={cn(
                  'w-full text-left px-3 py-2.5 rounded-lg transition-all duration-150',
                  'flex items-center gap-2 font-medium',
                  activeItem === item.id
                    ? 'bg-board-accent text-white shadow-sm'
                    : 'text-board-text-secondary hover:bg-board-card-hover hover:text-board-text'
                )}
              >
                {item.icon}
                {item.label}
              </button>
            </li>
          ))}
        </ul>
      </nav>
      <div className="pt-4 border-t border-board-border">
        <p className="text-xs text-board-text-muted">v0.1.0</p>
      </div>
    </aside>
  );
}
