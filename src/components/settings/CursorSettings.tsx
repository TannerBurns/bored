import { CursorIcon } from '../common';
import {
  AlertMessages,
  StatusSection,
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
      />

      <CursorInfoSections />
    </div>
  );
}
