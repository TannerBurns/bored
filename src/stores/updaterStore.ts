import { create } from 'zustand';
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

interface UpdaterState {
  state: UpdateState;
  isDismissed: boolean;
  isDownloading: boolean;
  
  // Actions
  checkForUpdates: () => Promise<void>;
  downloadAndInstall: (updateToInstall?: Update) => Promise<void>;
  handleRestart: () => Promise<void>;
  dismissUpdate: () => void;
  undoDismiss: () => void;
  reset: () => void;
}

export const useUpdaterStore = create<UpdaterState>()((set, get) => ({
  state: { status: 'idle' },
  isDismissed: false,
  isDownloading: false,

  checkForUpdates: async () => {
    try {
      set({ state: { status: 'checking' } });
      const update = await check();
      
      if (update) {
        // Check if this version was previously dismissed
        const dismissedVersion = getDismissedVersion();
        const isDismissed = dismissedVersion === update.version;
        set({ 
          state: { status: 'available', update },
          isDismissed,
        });
      } else {
        set({ state: { status: 'no-update' } });
      }
    } catch (error) {
      console.error('Failed to check for updates:', error);
      
      const errorMessage = error instanceof Error ? error.message : 'Failed to check for updates';
      
      // Handle case where no releases exist yet
      if (errorMessage.includes('Could not fetch a valid release')) {
        set({ state: { status: 'no-update' } });
        return;
      }
      
      let friendlyMessage = errorMessage;
      const lowerErrorMessage = errorMessage.toLowerCase();
      if (lowerErrorMessage.includes('network') || lowerErrorMessage.includes('fetch')) {
        friendlyMessage = 'Unable to connect. Check your internet connection.';
      }
      
      set({ 
        state: { status: 'error', message: friendlyMessage } 
      });
    }
  },

  downloadAndInstall: async (updateToInstall?: Update) => {
    const { state, isDownloading } = get();
    
    // Guard against concurrent invocations
    if (isDownloading) return;
    set({ isDownloading: true });
    
    const update = updateToInstall ?? (
      state.status === 'available' ? state.update : 
      state.status === 'error' && state.update ? state.update : 
      null
    );
    
    if (!update) {
      set({ isDownloading: false });
      return;
    }
    
    // Capture validated update in a const to ensure type safety throughout the function
    const validatedUpdate: Update = update;
    
    try {
      set({ 
        state: { 
          status: 'downloading', 
          progress: 0, 
          downloaded: 0, 
          contentLength: null, 
          update: validatedUpdate 
        } 
      });
      
      await validatedUpdate.downloadAndInstall((event: DownloadEvent) => {
        switch (event.event) {
          case 'Started':
            set({ 
              state: { 
                status: 'downloading', 
                progress: 0, 
                downloaded: 0, 
                contentLength: event.data.contentLength ?? null, 
                update: validatedUpdate 
              }
            });
            break;
          case 'Progress': {
            const chunkLength = event.data.chunkLength;
            const currentState = get().state;
            if (currentState.status !== 'downloading') return;
            
            const newDownloaded = currentState.downloaded + chunkLength;
            const progress = currentState.contentLength 
              ? Math.round((newDownloaded / currentState.contentLength) * 100)
              : 0;
            
            set({ 
              state: { 
                status: 'downloading', 
                progress: Math.min(progress, 100), 
                downloaded: newDownloaded,
                contentLength: currentState.contentLength,
                update: currentState.update 
              }
            });
            break;
          }
          case 'Finished':
            // Clear dismissed version when update is ready
            clearDismissedVersion();
            set({ 
              state: { status: 'ready', update: validatedUpdate },
              isDismissed: false,
            });
            break;
        }
      });
    } catch (error) {
      console.error('Failed to download update:', error);
      set({ 
        state: { 
          status: 'error', 
          message: error instanceof Error ? error.message : 'Failed to download update',
          update: validatedUpdate
        }
      });
    } finally {
      set({ isDownloading: false });
    }
  },

  handleRestart: async () => {
    try {
      await relaunch();
    } catch (error) {
      console.error('Failed to relaunch:', error);
    }
  },

  dismissUpdate: () => {
    const { state } = get();
    if (state.status === 'available') {
      setDismissedVersionStorage(state.update.version);
      set({ isDismissed: true });
    }
  },

  undoDismiss: () => {
    clearDismissedVersion();
    set({ isDismissed: false });
  },

  reset: () => {
    set({ state: { status: 'idle' } });
  },
}));
