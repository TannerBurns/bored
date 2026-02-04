import { useUpdaterStore, type UpdateState } from '../stores/updaterStore';

// Re-export the type for backward compatibility
export type { UpdateState };

/**
 * Hook for accessing the shared updater state.
 * Uses a Zustand store so all components share the same state.
 */
export function useUpdater() {
  const state = useUpdaterStore((s) => s.state);
  const isDismissed = useUpdaterStore((s) => s.isDismissed);
  const checkForUpdates = useUpdaterStore((s) => s.checkForUpdates);
  const downloadAndInstall = useUpdaterStore((s) => s.downloadAndInstall);
  const handleRestart = useUpdaterStore((s) => s.handleRestart);
  const dismissUpdate = useUpdaterStore((s) => s.dismissUpdate);
  const undoDismiss = useUpdaterStore((s) => s.undoDismiss);
  const reset = useUpdaterStore((s) => s.reset);

  return {
    state,
    isDismissed,
    checkForUpdates,
    downloadAndInstall,
    handleRestart,
    dismissUpdate,
    undoDismiss,
    reset,
  };
}
