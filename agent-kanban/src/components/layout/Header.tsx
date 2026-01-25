interface HeaderProps {
  title: string;
  subtitle?: string;
}

export function Header({ title, subtitle }: HeaderProps) {
  return (
    <header className="mb-6">
      <h2 className="text-2xl font-semibold text-white">{title}</h2>
      {subtitle && (
        <p className="text-gray-400 mt-1">{subtitle}</p>
      )}
    </header>
  );
}
