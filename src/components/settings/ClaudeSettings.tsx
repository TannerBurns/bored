import { ClaudeIcon } from '../common';
import {
  AlertMessages,
  StatusSection,
  HookScriptSection,
  InstallHooksSection,
  InstallCommandsSection,
} from './shared';
import {
  useClaudeSettings,
  ApiConfigSection,
  ClaudeInfoSections,
} from './claude';

export function ClaudeSettings() {
  const claude = useClaudeSettings();

  if (claude.loading) {
    return (
      <div className="text-board-text-muted text-center py-8">
        Loading Claude status...
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <h2 className="text-lg font-semibold text-board-text flex items-center gap-2">
        <ClaudeIcon size={20} className="text-[#da7756]" />
        Claude Code Integration
      </h2>

      <AlertMessages error={claude.error} success={claude.success} />

      <StatusSection
        isAvailable={claude.status?.isAvailable ?? false}
        version={claude.status?.version}
        hooksInstalled={claude.userHooksInstalled}
        commandsInstalled={claude.userCommandsInstalled}
        hooksLabel="User hooks"
      />

      <ApiConfigSection apiSettings={claude.apiSettings} />

      <HookScriptSection
        hookPath={claude.status?.hookScriptPath}
        onCopy={claude.handleCopyPath}
        description="Intercepts Claude Code lifecycle events."
      />

      <InstallHooksSection
        location={claude.hookInstall.location}
        onLocationChange={claude.hookInstall.setLocation}
        projects={claude.projects}
        selectedProjectId={claude.hookInstall.selectedProjectId}
        onProjectSelect={claude.hookInstall.setSelectedProjectId}
        projectPath={claude.hookInstall.projectPath}
        onPathChange={claude.hookInstall.setProjectPath}
        onBrowse={() => claude.handleBrowse('hooks')}
        installing={claude.hookInstall.installing}
        onInstall={claude.hookInstall.install}
        onCopyConfig={claude.hookInstall.copyConfig}
        hookPathAvailable={!!claude.status?.hookScriptPath}
        labels={{ user: 'User (~/.claude/)', project: 'Project-specific' }}
        radioGroupName="claude-hook-location"
      />

      <InstallCommandsSection
        location={claude.commandInstall.location}
        onLocationChange={claude.commandInstall.setLocation}
        projects={claude.projects}
        projectId={claude.commandInstall.projectId}
        onProjectSelect={claude.commandInstall.setProjectId}
        projectPath={claude.commandInstall.projectPath}
        onPathChange={claude.commandInstall.setProjectPath}
        onBrowse={() => claude.handleBrowse('commands')}
        installing={claude.commandInstall.installing}
        onInstall={claude.commandInstall.install}
        availableCommands={claude.availableCommands}
        userCommandsInstalled={claude.userCommandsInstalled}
        projectCommandStatus={claude.projectCommandStatus}
        userLabel="User"
        radioGroupName="claude-command-location"
      />

      <ClaudeInfoSections
        configVisible={claude.configVisible}
        setConfigVisible={claude.setConfigVisible}
        configJson={claude.configJson}
      />
    </div>
  );
}
