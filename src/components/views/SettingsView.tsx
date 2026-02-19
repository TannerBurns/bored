import { useState, useEffect, useMemo } from 'react';
import { GeneralSettings, DataSettings } from '../settings';
import { AgentSettingsPage } from '../settings/AgentSettingsPage';
import { useAgentRegistryStore } from '../../stores/agentRegistryStore';
import { getAgentIcon, getAgentBrandColor } from '../common/AgentIcons';

const CORE_TABS = [
  { id: 'general', label: 'General' },
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
    [...agents]
      .sort((a, b) => a.displayName.localeCompare(b.displayName))
      .map((a) => ({ id: a.id, label: a.displayName, brandColor: a.brandColor })),
    [agents]
  );

  const allTabs = useMemo(() => [
    ...CORE_TABS,
    ...agentTabs,
    ...TRAILING_TABS,
  ], [agentTabs]);

  const isAgentTab = agents.some((a) => a.id === settingsTab);

  return (
    <div className="flex-1 overflow-hidden flex flex-col">
      <div className="flex gap-1 mb-3">
        {allTabs.map((tab) => {
          const agentInfo = agents.find((a) => a.id === tab.id);
          const isSelected = settingsTab === tab.id;
          return (
            <button
              key={tab.id}
              onClick={() => setSettingsTab(tab.id)}
              className={`px-3 py-1.5 text-sm font-medium rounded-lg transition-all duration-200 flex items-center gap-1.5 ${
                isSelected
                  ? 'bg-board-accent text-white shadow-sm'
                  : 'glass text-board-text-muted hover:text-board-text hover:bg-board-card-hover'
              }`}
            >
              {agentInfo && (() => {
                const Icon = getAgentIcon(agentInfo.id);
                const color = isSelected ? undefined : getAgentBrandColor(agentInfo.id, agentInfo.brandColor);
                return <Icon size={14} style={color ? { color } : undefined} />;
              })()}
              {tab.label}
            </button>
          );
        })}
      </div>
      
      <div className="flex-1 overflow-auto glass rounded-lg p-4">
        {settingsTab === 'general' && <GeneralSettings onShowReleaseNotes={onShowReleaseNotes} />}
        {isAgentTab && <AgentSettingsPage agentId={settingsTab} />}
        {settingsTab === 'data' && <DataSettings />}
      </div>
    </div>
  );
}
