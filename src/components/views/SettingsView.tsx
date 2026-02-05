import { useState } from 'react';
import { CursorSettings, ClaudeSettings, GeneralSettings, DataSettings } from '../settings';

type SettingsTab = 'general' | 'cursor' | 'claude' | 'data';

const SETTINGS_TABS = [
  { id: 'general', label: 'General' },
  { id: 'cursor', label: 'Cursor' },
  { id: 'claude', label: 'Claude Code' },
  { id: 'data', label: 'Data' },
] as const;

export function SettingsView() {
  const [settingsTab, setSettingsTab] = useState<SettingsTab>('general');

  return (
    <div className="flex-1 overflow-hidden flex flex-col">
      <div className="flex gap-1 mb-3">
        {SETTINGS_TABS.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setSettingsTab(tab.id)}
            className={`px-3 py-1.5 text-sm font-medium rounded-lg transition-all duration-200 ${
              settingsTab === tab.id
                ? 'bg-board-accent text-white shadow-sm'
                : 'glass text-board-text-muted hover:text-board-text hover:bg-board-card-hover'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>
      
      <div className="flex-1 overflow-auto glass rounded-lg p-4">
        {settingsTab === 'general' && <GeneralSettings />}
        {settingsTab === 'cursor' && <CursorSettings />}
        {settingsTab === 'claude' && <ClaudeSettings />}
        {settingsTab === 'data' && <DataSettings />}
      </div>
    </div>
  );
}
