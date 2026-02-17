import { SpecAgentSettings } from './SpecAgentSettings';
import { ValidationAgentSettings } from './ValidationAgentSettings';
import { DiagnosticAgentSettings } from './DiagnosticAgentSettings';

export function AgentsSettings() {
  return (
    <div className="space-y-6">
      <SpecAgentSettings />
      <hr className="border-board-border/30" />
      <ValidationAgentSettings />
      <hr className="border-board-border/30" />
      <DiagnosticAgentSettings />
    </div>
  );
}
