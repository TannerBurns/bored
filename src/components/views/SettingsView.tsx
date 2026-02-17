import { useState, useEffect, useMemo } from 'react';
import { CursorSettings, ClaudeSettings, GeneralSettings, AgentWorkflowSettings, AgentsSettings, DataSettings } from '../settings';
import { useAgentRegistryStore } from '../../stores/agentRegistryStore';

const AGENT_SETTINGS_COMPONENTS: Record<string, React.ComponentType> = {
  cursor: CursorSettings,
  claude: ClaudeSettings,
};

const CORE_TABS = [
  { id: 'general', label: 'General' },
  { id: 'workflow', label: 'Agent Workflow' },
  { id: 'agents', label: 'Agents' },
] as const;

const TRAILING_TABS = [
  { id: 'data', label: 'Data' },
] as const;

interface SettingsViewProps {
  onShowReleaseNotes: () => void;
}

export function SettingsView({ onShowReleaseNotes }: SettingsViewProps) {
  const [settingsTab, setSettingsTab] = useState<string>('general');
  const agents = useAgentRegistryStore((s) => s.agents);
  const loadAgents = useAgentRegistryStore((s) => s.loadAgents);

  useEffect(() => {
    loadAgents();
  }, [loadAgents]);

  const agentTabs = useMemo(() =>
    agents
      .filter((a) => AGENT_SETTINGS_COMPONENTS[a.id])
      .map((a) => ({ id: a.id, label: a.displayName })),
    [agents]
  );

  const allTabs = useMemo(() => [
    ...CORE_TABS,
    ...agentTabs,
    ...TRAILING_TABS,
  ], [agentTabs]);

  const AgentSettingsComponent = AGENT_SETTINGS_COMPONENTS[settingsTab];

  return (
    <div className="flex-1 overflow-hidden flex flex-col">
      <div className="flex gap-1 mb-3">
        {allTabs.map((tab) => (
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
        {settingsTab === 'agents' && <AgentsSettings />}
        {AgentSettingsComponent && <AgentSettingsComponent />}
        {settingsTab === 'data' && <DataSettings />}
      </div>
    </div>
  );
}
