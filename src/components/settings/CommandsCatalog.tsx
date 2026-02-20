import { useState, useCallback, useEffect } from 'react';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { useSettingsStore, type CatalogCommand } from '../../stores/settingsStore';
import { readCommandContent, saveCustomCommand } from '../../lib/tauri';
import { cn } from '../../lib/utils';

function EnabledToast({ commandName, onDismiss }: { commandName: string; onDismiss: () => void }) {
  useEffect(() => {
    const timer = setTimeout(onDismiss, 5000);
    return () => clearTimeout(timer);
  }, [onDismiss]);

  return (
    <div className="glass-intense rounded-lg border border-board-accent/30 px-3 py-2.5 flex items-start gap-2.5 animate-in slide-in-from-top-1 fade-in-0 duration-200">
      <svg className="w-4 h-4 text-board-accent flex-shrink-0 mt-0.5" viewBox="0 0 16 16" fill="currentColor">
        <path d="M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1Zm-.75 3.75a.75.75 0 0 1 1.5 0v4a.75.75 0 0 1-1.5 0v-4ZM8 11.5A.75.75 0 1 1 8 10a.75.75 0 0 1 0 1.5Z" />
      </svg>
      <div className="flex-1 min-w-0">
        <p className="text-xs text-board-text">
          <span className="font-medium">{commandName}</span> has been added to all agent workflows.
        </p>
        <p className="text-[11px] text-board-text-muted mt-0.5">
          Go to each agent tab to configure stage ordering and model selection.
        </p>
      </div>
      <button
        onClick={onDismiss}
        className="p-0.5 text-board-text-muted hover:text-board-text transition-colors flex-shrink-0"
        aria-label="Dismiss"
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5">
          <path d="M3 3l6 6M9 3l-6 6" />
        </svg>
      </button>
    </div>
  );
}

function CommandCard({
  command,
  onToggle,
  onDelete,
}: {
  command: CatalogCommand;
  onToggle: (id: string) => void;
  onDelete?: (id: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [content, setContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleExpand = useCallback(async () => {
    if (!expanded && content === null) {
      setLoading(true);
      try {
        const md = await readCommandContent(command.filename);
        setContent(md);
      } catch {
        setContent('*Failed to load command content.*');
      } finally {
        setLoading(false);
      }
    }
    setExpanded(!expanded);
  }, [expanded, content, command.filename]);

  return (
    <div className={cn(
      'glass rounded-lg transition-all duration-200',
      command.enabled ? 'ring-1 ring-board-accent/20' : 'opacity-70',
    )}>
      <div className="flex items-center gap-3 px-3 py-2.5">
        <button
          onClick={() => onToggle(command.id)}
          className={cn(
            'relative inline-flex h-5 w-9 flex-shrink-0 rounded-full transition-colors duration-200 cursor-pointer',
            command.enabled ? 'bg-board-accent' : 'glass-intense',
          )}
          title={`${command.enabled ? 'Disable' : 'Enable'} ${command.name}`}
        >
          <span className={cn(
            'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow transition duration-200',
            command.enabled ? 'translate-x-4' : 'translate-x-0.5',
          )} style={{ marginTop: '2px' }} />
        </button>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-board-text">{command.name}</span>
            <span className={cn(
              'text-[9px] font-medium px-1.5 py-0 rounded-full leading-relaxed',
              command.source === 'builtin'
                ? 'bg-board-accent/15 text-board-accent'
                : 'bg-emerald-500/15 text-emerald-400',
            )}>
              {command.source}
            </span>
          </div>
          <p className="text-[11px] text-board-text-muted truncate">{command.description}</p>
        </div>

        <div className="flex items-center gap-1">
          <button
            onClick={handleExpand}
            className="p-1 text-board-text-muted hover:text-board-text transition-colors rounded"
            title={expanded ? 'Collapse' : 'View command'}
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5">
              {expanded
                ? <path d="M4 9l3-3 3 3" />
                : <path d="M4 5l3 3 3-3" />
              }
            </svg>
          </button>
          {onDelete && (
            <button
              onClick={() => onDelete(command.id)}
              className="p-1 text-board-text-muted hover:text-red-400 transition-colors rounded"
              title="Delete command"
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M3 4h8M5.5 4V3a1 1 0 011-1h1a1 1 0 011 1v1M6 6.5v3M8 6.5v3M4.5 4l.5 7a1 1 0 001 1h2a1 1 0 001-1l.5-7" />
              </svg>
            </button>
          )}
        </div>
      </div>

      {expanded && (
        <div className="border-t border-board-border/20 px-3 py-2 max-h-64 overflow-y-auto">
          {loading ? (
            <p className="text-xs text-board-text-muted italic">Loading...</p>
          ) : (
            <MarkdownViewer content={content ?? ''} className="text-xs" />
          )}
        </div>
      )}
    </div>
  );
}

function AddCommandForm({ onSave, onCancel }: {
  onSave: (name: string, description: string, content: string) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [content, setContent] = useState('');
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    if (!name.trim() || !content.trim()) return;
    setSaving(true);
    try {
      await onSave(name.trim(), description.trim(), content.trim());
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="glass rounded-lg p-3 space-y-3">
      <h4 className="text-sm font-medium text-board-text">New Custom Command</h4>
      <div className="space-y-2">
        <input
          type="text"
          placeholder="Command name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="w-full px-2.5 py-1.5 text-sm glass rounded-lg text-board-text placeholder:text-board-text-muted/50 focus:ring-1 focus:ring-board-accent"
        />
        <input
          type="text"
          placeholder="Short description"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          className="w-full px-2.5 py-1.5 text-sm glass rounded-lg text-board-text placeholder:text-board-text-muted/50 focus:ring-1 focus:ring-board-accent"
        />
        <textarea
          placeholder="Command instructions (markdown)"
          value={content}
          onChange={(e) => setContent(e.target.value)}
          rows={8}
          className="w-full px-2.5 py-1.5 text-sm glass rounded-lg text-board-text placeholder:text-board-text-muted/50 focus:ring-1 focus:ring-board-accent resize-y font-mono"
        />
      </div>
      <div className="flex gap-2 justify-end">
        <button
          onClick={onCancel}
          className="px-3 py-1.5 text-xs font-medium glass rounded-lg text-board-text-muted hover:text-board-text transition-colors"
        >
          Cancel
        </button>
        <button
          onClick={handleSave}
          disabled={!name.trim() || !content.trim() || saving}
          className="px-3 py-1.5 text-xs font-medium bg-board-accent text-white rounded-lg hover:bg-board-accent/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {saving ? 'Saving...' : 'Save Command'}
        </button>
      </div>
    </div>
  );
}

export function CommandsCatalog() {
  const catalog = useSettingsStore((s) => s.commandsCatalog);
  const toggleCommand = useSettingsStore((s) => s.toggleCatalogCommand);
  const addCommand = useSettingsStore((s) => s.addCustomCommand);
  const removeCommand = useSettingsStore((s) => s.removeCustomCommand);
  const [showAddForm, setShowAddForm] = useState(false);
  const [enabledToast, setEnabledToast] = useState<string | null>(null);

  const builtinCommands = catalog.filter((c) => c.source === 'builtin');
  const customCommands = catalog.filter((c) => c.source === 'custom');

  useEffect(() => {
    const enabledFilenames = catalog
      .filter((c) => c.enabled)
      .map((c) => c.filename);
    const disabledFilenames = catalog
      .filter((c) => !c.enabled)
      .map((c) => c.filename);

    if (enabledFilenames.length > 0 || disabledFilenames.length > 0) {
      import('../../lib/tauri').then(({ installCatalogCommandsToAllProjects }) => {
        installCatalogCommandsToAllProjects(enabledFilenames, disabledFilenames).catch(() => {});
      });
    }
  }, [catalog]);

  const handleToggle = useCallback((id: string) => {
    const cmd = catalog.find((c) => c.id === id);
    const wasEnabled = cmd?.enabled ?? false;
    toggleCommand(id);
    if (!wasEnabled) {
      setEnabledToast(cmd?.name ?? id);
    }
  }, [catalog, toggleCommand]);

  const handleAddCommand = useCallback(async (name: string, description: string, content: string) => {
    const id = name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
    const filename = `${id}.md`;

    try {
      await saveCustomCommand(id, filename, content);
    } catch (err) {
      console.warn('Failed to save custom command file:', err);
    }

    addCommand({
      id,
      name,
      description,
      enabled: true,
      source: 'custom',
      filename,
    });
    setShowAddForm(false);
    setEnabledToast(name);
  }, [addCommand]);

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-board-text">Commands Catalog</h2>
        <p className="text-xs text-board-text-muted mt-0.5">
          Manage the command library. Enabled commands appear as workflow stages for all agents.
        </p>
      </div>

      {enabledToast && (
        <EnabledToast
          commandName={enabledToast}
          onDismiss={() => setEnabledToast(null)}
        />
      )}

      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h3 className="text-sm font-medium text-board-text">Built-in Commands</h3>
          <p className="text-xs text-board-text-muted mt-0.5">
            Standard commands bundled with the application. Toggle to add or remove from workflows.
          </p>
        </div>
        <div className="space-y-1.5">
          {builtinCommands.map((cmd) => (
            <CommandCard key={cmd.id} command={cmd} onToggle={handleToggle} />
          ))}
        </div>
      </div>

      <div className="glass rounded-lg p-3 space-y-3">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-sm font-medium text-board-text">Custom Commands</h3>
            <p className="text-xs text-board-text-muted mt-0.5">
              Your own command instructions. Write markdown to define custom workflow stages.
            </p>
          </div>
          {!showAddForm && (
            <button
              onClick={() => setShowAddForm(true)}
              className="px-2.5 py-1 text-xs font-medium bg-board-accent text-white rounded-lg hover:bg-board-accent/90 transition-colors"
            >
              + Add Command
            </button>
          )}
        </div>

        {customCommands.length > 0 && (
          <div className="space-y-1.5">
            {customCommands.map((cmd) => (
              <CommandCard
                key={cmd.id}
                command={cmd}
                onToggle={handleToggle}
                onDelete={removeCommand}
              />
            ))}
          </div>
        )}

        {customCommands.length === 0 && !showAddForm && (
          <p className="text-xs text-board-text-muted italic py-2">
            No custom commands yet. Add one to create your own workflow stages.
          </p>
        )}

        {showAddForm && (
          <AddCommandForm
            onSave={handleAddCommand}
            onCancel={() => setShowAddForm(false)}
          />
        )}
      </div>
    </div>
  );
}
