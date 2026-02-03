import { CursorIcon } from '../common';
import {
  AlertMessages,
  StatusSection,
  HookScriptSection,
  InstallHooksSection,
  InstallCommandsSection,
} from './shared';
import { useCursorSettings, CursorInfoSections } from './cursor';

export function CursorSettings() {
  const cursor = useCursorSettings();

  if (cursor.loading) {
    return (
      <div className="text-board-text-muted text-center py-8">
        Loading Cursor status...
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <h2 className="text-lg font-semibold text-board-text flex items-center gap-2">
        <CursorIcon size={20} className="text-board-text" />
        Cursor Integration
      </h2>

      <AlertMessages error={cursor.error} success={cursor.success} />

      <StatusSection
        isAvailable={cursor.status?.isAvailable ?? false}
        version={cursor.status?.version}
        hooksInstalled={cursor.globalHooksInstalled}
        commandsInstalled={cursor.userCommandsInstalled}
        hooksLabel="Global hooks"
      />

      <HookScriptSection
        hookPath={cursor.status?.hookScriptPath}
        onCopy={cursor.handleCopyPath}
        description="Intercepts Cursor agent events and sends them to Agent Kanban."
      />

      <InstallHooksSection
        location={cursor.hookInstall.location}
        onLocationChange={cursor.hookInstall.setLocation}
        projects={cursor.projects}
        selectedProjectId={cursor.hookInstall.selectedProjectId}
        onProjectSelect={cursor.hookInstall.setSelectedProjectId}
        projectPath={cursor.hookInstall.projectPath}
        onPathChange={cursor.hookInstall.setProjectPath}
        onBrowse={() => cursor.handleBrowse('hooks')}
        installing={cursor.hookInstall.installing}
        onInstall={cursor.hookInstall.install}
        onCopyConfig={cursor.hookInstall.copyConfig}
        hookPathAvailable={!!cursor.status?.hookScriptPath}
        labels={{ user: 'Global', project: 'Project-specific' }}
        radioGroupName="cursor-hook-location"
      />

      <InstallCommandsSection
        location={cursor.commandInstall.location}
        onLocationChange={cursor.commandInstall.setLocation}
        projects={cursor.projects}
        projectId={cursor.commandInstall.projectId}
        onProjectSelect={cursor.commandInstall.setProjectId}
        projectPath={cursor.commandInstall.projectPath}
        onPathChange={cursor.commandInstall.setProjectPath}
        onBrowse={() => cursor.handleBrowse('commands')}
        installing={cursor.commandInstall.installing}
        onInstall={cursor.commandInstall.install}
        availableCommands={cursor.availableCommands}
        userCommandsInstalled={cursor.userCommandsInstalled}
        projectCommandStatus={cursor.projectCommandStatus}
        userLabel="Global"
        radioGroupName="cursor-command-location"
        showSlashPrefix
      />

      <CursorInfoSections
        hookPath={cursor.status?.hookScriptPath}
        configVisible={cursor.configVisible}
        setConfigVisible={cursor.setConfigVisible}
        configJson={cursor.configJson}
      />
    </div>
  );
}
