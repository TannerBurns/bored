import { useState, useEffect, useCallback } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { getReleaseNotes, getAllReleaseNotes } from '../lib/tauri';
import type { ReleaseNote } from '../types';

const LAST_SEEN_VERSION_KEY = 'release-notes-last-seen-version';

function getLastSeenVersion(): string | null {
  try {
    return localStorage.getItem(LAST_SEEN_VERSION_KEY);
  } catch {
    return null;
  }
}

function setLastSeenVersion(version: string): void {
  try {
    localStorage.setItem(LAST_SEEN_VERSION_KEY, version);
  } catch {
    // Ignore storage errors
  }
}

/**
 * Try to fetch release notes for the exact version first.
 * If no exact match, fall back to the most recent available notes.
 */
async function fetchBestReleaseNotes(version: string): Promise<ReleaseNote | null> {
  // Try exact version match first
  const exact = await getReleaseNotes(version);
  if (exact) return exact;

  // Fall back to the latest available release notes
  const all = await getAllReleaseNotes();
  return all.length > 0 ? all[0] : null;
}

interface UseReleaseNotesResult {
  /** Whether the release notes modal should be shown (auto-triggered on upgrade) */
  isOpen: boolean;
  /** The release notes data (null if not loaded or no notes for current version) */
  releaseNote: ReleaseNote | null;
  /** Current app version */
  appVersion: string;
  /** Dismiss the modal and mark the current version as seen */
  dismiss: () => void;
  /** Manually open the release notes modal (e.g., from About section) */
  showReleaseNotes: () => void;
}

export function useReleaseNotes(): UseReleaseNotesResult {
  const [isOpen, setIsOpen] = useState(false);
  const [releaseNote, setReleaseNote] = useState<ReleaseNote | null>(null);
  const [appVersion, setAppVersion] = useState('');

  // On mount: check if the user has seen the current version's release notes
  useEffect(() => {
    let cancelled = false;

    async function checkForNewVersion() {
      try {
        const version = await getVersion();
        if (cancelled) return;
        setAppVersion(version);

        const lastSeen = getLastSeenVersion();
        if (lastSeen === version) return;

        // Version is different — fetch release notes (exact match or latest)
        const notes = await fetchBestReleaseNotes(version);
        if (cancelled) return;

        if (notes) {
          setReleaseNote(notes);
          setIsOpen(true);
        } else {
          // No release notes at all — mark as seen silently
          setLastSeenVersion(version);
        }
      } catch (error) {
        console.error('Failed to check release notes:', error);
      }
    }

    // Delay slightly to not block app startup
    const timer = setTimeout(checkForNewVersion, 1500);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, []);

  const dismiss = useCallback(() => {
    setIsOpen(false);
    if (appVersion) {
      setLastSeenVersion(appVersion);
    }
  }, [appVersion]);

  const showReleaseNotes = useCallback(async () => {
    try {
      const version = appVersion || (await getVersion());
      if (!appVersion) setAppVersion(version);

      // For manual viewing, show exact match or latest available
      const notes = await fetchBestReleaseNotes(version);
      if (notes) {
        setReleaseNote(notes);
        setIsOpen(true);
      }
    } catch (error) {
      console.error('Failed to load release notes:', error);
    }
  }, [appVersion]);

  return {
    isOpen,
    releaseNote,
    appVersion,
    dismiss,
    showReleaseNotes,
  };
}
