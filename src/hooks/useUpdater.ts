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
    
    // Capture validated update in a const to ensure type safety throughout the function
    const validatedUpdate: Update = update;
    
    try {
      setState({ status: 'downloading', progress: 0, downloaded: 0, contentLength: null, update: validatedUpdate });
      
      await validatedUpdate.downloadAndInstall((event: DownloadEvent) => {
        switch (event.event) {
          case 'Started':
            setState({ 
              status: 'downloading', 
              progress: 0, 
              downloaded: 0, 
              contentLength: event.data.contentLength ?? null, 
              update: validatedUpdate 
            });
            break;
          case 'Progress': {
            // Capture event data outside setState to avoid closure issues with rapid calls
            const chunkLength = event.data.chunkLength;
            setState((prev) => {
              if (prev.status !== 'downloading') return prev;
              
              const newDownloaded = prev.downloaded + chunkLength;
              const progress = prev.contentLength 
                ? Math.round((newDownloaded / prev.contentLength) * 100)
                : 0;
              
              // Use prev.update to ensure consistency with current state
              return { 
                status: 'downloading', 
                progress: Math.min(progress, 100), 
                downloaded: newDownloaded,
                contentLength: prev.contentLength,
                update: prev.update 
              };
            });
            break;
          }
          case 'Finished':
            // Clear dismissed version when update is ready
            clearDismissedVersion();
            setState({ status: 'ready', update: validatedUpdate });
            break;
        }
      });
    } catch (error) {
      console.error('Failed to download update:', error);
      setState({ 
        status: 'error', 
        message: error instanceof Error ? error.message : 'Failed to download update',
        update: validatedUpdate
      });
    } finally {
      // Always reset the guard to allow future download attempts,
      // regardless of success, failure, or if 'Finished' event was never emitted
      isDownloadingRef.current = false;
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
