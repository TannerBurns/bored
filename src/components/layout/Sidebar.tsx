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
    <aside className="w-64 bg-board-column border-r border-gray-700 p-4 flex flex-col">
      <h1 className="text-xl font-bold text-white mb-6">Agent Kanban</h1>
      <nav className="flex-1">
        <ul className="space-y-2">
          {navItems.map((item) => (
            <li key={item.id}>
              <button
                onClick={() => onItemClick(item.id)}
                className={cn(
                  'w-full text-left px-3 py-2 rounded transition-colors',
                  'flex items-center gap-2',
                  activeItem === item.id
                    ? 'bg-board-card text-white'
                    : 'text-gray-300 hover:bg-board-card hover:text-white'
                )}
              >
                {item.icon}
                {item.label}
              </button>
            </li>
          ))}
        </ul>
      </nav>
      <div className="pt-4 border-t border-gray-700">
        <p className="text-xs text-gray-500">v0.1.0</p>
      </div>
    </aside>
  );
}
