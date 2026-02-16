import { ClaudeIcon } from '../common';
import {
  AlertMessages,
  StatusSection,
} from './shared';
import {
  useClaudeSettings,
  ApiConfigSection,
  CliOptionsSection,
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
      />

      <CliOptionsSection cliOptions={claude.cliOptions} />

      <ApiConfigSection apiSettings={claude.apiSettings} />

      <ClaudeInfoSections />
    </div>
  );
}
