import { useState } from 'react';
import { CursorSettings, ClaudeSettings, GeneralSettings, AgentWorkflowSettings, SpecAgentSettings, ValidationAgentSettings, DataSettings } from '../settings';

type SettingsTab = 'general' | 'workflow' | 'spec-agent' | 'validation-agent' | 'cursor' | 'claude' | 'data';

const SETTINGS_TABS = [
  { id: 'general', label: 'General' },
  { id: 'workflow', label: 'Agent Workflow' },
  { id: 'spec-agent', label: 'Spec Agent' },
  { id: 'validation-agent', label: 'Validation Agent' },
  { id: 'cursor', label: 'Cursor' },
  { id: 'claude', label: 'Claude Code' },
  { id: 'data', label: 'Data' },
] as const;

interface SettingsViewProps {
  onShowReleaseNotes: () => void;
}

export function SettingsView({ onShowReleaseNotes }: SettingsViewProps) {
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
        {settingsTab === 'general' && <GeneralSettings onShowReleaseNotes={onShowReleaseNotes} />}
        {settingsTab === 'workflow' && <AgentWorkflowSettings />}
        {settingsTab === 'spec-agent' && <SpecAgentSettings />}
        {settingsTab === 'validation-agent' && <ValidationAgentSettings />}
        {settingsTab === 'cursor' && <CursorSettings />}
        {settingsTab === 'claude' && <ClaudeSettings />}
        {settingsTab === 'data' && <DataSettings />}
      </div>
    </div>
  );
}
