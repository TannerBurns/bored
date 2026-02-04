import { useState, useCallback, useRef } from 'react';
import { check, type Update, type DownloadEvent } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

export type UpdateState = 
  | { status: 'idle' }
  | { status: 'checking' }
  | { status: 'no-update' }
  | { status: 'available'; update: Update }
  | { status: 'downloading'; progress: number; downloaded: number; contentLength: number | null; update: Update }
  | { status: 'ready'; update: Update }
  | { status: 'error'; message: string; update?: Update };

const DISMISSED_VERSION_KEY = 'update-notification-dismissed-version';

function getDismissedVersion(): string | null {
  try {
    return localStorage.getItem(DISMISSED_VERSION_KEY);
  } catch {
    return null;
  }
}

function setDismissedVersionStorage(version: string): void {
  try {
    localStorage.setItem(DISMISSED_VERSION_KEY, version);
  } catch {
    // Ignore storage errors
  }
}

function clearDismissedVersion(): void {
  try {
    localStorage.removeItem(DISMISSED_VERSION_KEY);
  } catch {
    // Ignore storage errors
  }
}

export function useUpdater() {
  const [state, setState] = useState<UpdateState>({ status: 'idle' });
  const [isDismissed, setIsDismissed] = useState(false);
  
  // Ref to track current state for stable callbacks
  const stateRef = useRef(state);
  stateRef.current = state;
  
  // Ref to guard against concurrent download invocations
  const isDownloadingRef = useRef(false);

  const checkForUpdates = useCallback(async () => {
    try {
      setState({ status: 'checking' });
      const update = await check();
      
      if (update) {
        // Check if this version was previously dismissed
        const dismissedVersion = getDismissedVersion();
        if (dismissedVersion === update.version) {
          setIsDismissed(true);
        } else {
          setIsDismissed(false);
        }
        setState({ status: 'available', update });
      } else {
        setState({ status: 'no-update' });
      }
    } catch (error) {
      console.error('Failed to check for updates:', error);
      
      const errorMessage = error instanceof Error ? error.message : 'Failed to check for updates';
      
      // Handle case where no releases exist yet
      if (errorMessage.includes('Could not fetch a valid release')) {
        setState({ status: 'no-update' });
        return;
      }
      
      let friendlyMessage = errorMessage;
      const lowerErrorMessage = errorMessage.toLowerCase();
      if (lowerErrorMessage.includes('network') || lowerErrorMessage.includes('fetch')) {
        friendlyMessage = 'Unable to connect. Check your internet connection.';
      }
      
      setState({ 
        status: 'error', 
        message: friendlyMessage 
      });
    }
  }, []);

  const downloadAndInstall = useCallback(async (updateToInstall?: Update) => {
    // Guard against concurrent invocations - set immediately after check to prevent race conditions
    if (isDownloadingRef.current) return;
    isDownloadingRef.current = true;
    
    // Use ref to access current state without creating dependency
    const currentState = stateRef.current;
    const update = updateToInstall ?? (
      currentState.status === 'available' ? currentState.update : 
      currentState.status === 'error' && currentState.update ? currentState.update : 
      null
    );
    if (!update) {
      isDownloadingRef.current = false;
      return;
    }
    
    try {
      setState({ status: 'downloading', progress: 0, downloaded: 0, contentLength: null, update });
      
      await update.downloadAndInstall((event: DownloadEvent) => {
        switch (event.event) {
          case 'Started':
            setState({ 
              status: 'downloading', 
              progress: 0, 
              downloaded: 0, 
              contentLength: event.data.contentLength ?? null, 
              update 
            });
            break;
          case 'Progress':
            setState((prev) => {
              if (prev.status !== 'downloading') return prev;
              
              const newDownloaded = prev.downloaded + event.data.chunkLength;
              const progress = prev.contentLength 
                ? Math.round((newDownloaded / prev.contentLength) * 100)
                : 0;
              
              return { 
                status: 'downloading', 
                progress: Math.min(progress, 100), 
                downloaded: newDownloaded,
                contentLength: prev.contentLength,
                update 
              };
            });
            break;
          case 'Finished':
            // Clear dismissed version when update is ready
            clearDismissedVersion();
            isDownloadingRef.current = false;
            setState({ status: 'ready', update });
            break;
        }
      });
    } catch (error) {
      console.error('Failed to download update:', error);
      isDownloadingRef.current = false;
      setState({ 
        status: 'error', 
        message: error instanceof Error ? error.message : 'Failed to download update',
        update
      });
    }
  }, []);

  const handleRestart = useCallback(async () => {
    try {
      await relaunch();
    } catch (error) {
      console.error('Failed to relaunch:', error);
    }
  }, []);

  const dismissUpdate = useCallback(() => {
    // Use ref to access current state without creating dependency
    const currentState = stateRef.current;
    if (currentState.status === 'available') {
      setDismissedVersionStorage(currentState.update.version);
      setIsDismissed(true);
    }
  }, []);

  const undoDismiss = useCallback(() => {
    clearDismissedVersion();
    setIsDismissed(false);
  }, []);

  const reset = useCallback(() => {
    setState({ status: 'idle' });
  }, []);

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
