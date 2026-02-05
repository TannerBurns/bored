import { useState, useEffect } from 'react';
import { getCursorStatus, getClaudeStatus } from '../lib/tauri';

interface CliAvailability {
  cursorAvailable: boolean;
  claudeAvailable: boolean;
  loading: boolean;
}

/**
 * Hook to check CLI availability for Cursor and Claude agents.
 * Returns availability status for both CLIs, defaulting to unavailable on error.
 */
export function useCliAvailability(): CliAvailability {
  const [cursorAvailable, setCursorAvailable] = useState<boolean>(false);
  const [claudeAvailable, setClaudeAvailable] = useState<boolean>(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const checkAvailability = async () => {
      try {
        const [cursorStatus, claudeStatus] = await Promise.all([
          getCursorStatus(),
          getClaudeStatus(),
        ]);
        setCursorAvailable(cursorStatus.isAvailable);
        setClaudeAvailable(claudeStatus.isAvailable);
      } catch {
        setCursorAvailable(false);
        setClaudeAvailable(false);
      } finally {
        setLoading(false);
      }
    };
    checkAvailability();
  }, []);

  return { cursorAvailable, claudeAvailable, loading };
}
