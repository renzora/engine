import { onMount, For } from 'solid-js'
import './base.css'
import './themes'
import { Engine, layoutComponents } from '@/api/plugin'
import Layout from './layout'
import DevNotice from './components/DevNotice'
import EditorPage from './pages/editor'
import { Project } from './plugins/core/splash/ProjectStore'
import KeyboardShortcuts from './components/KeyboardShortcuts'
import CloseConfirmationOverlay from '@/ui/CloseConfirmationOverlay.jsx'
import { closeConfirmationStore } from '@/stores/CloseConfirmationStore.jsx'
export default function App() {
  onMount(async () => {
    // Engine loaded successfully
    
    // Setup window close handling for Tauri (handles Alt+F4, system close, etc.)
    const setupWindowCloseHandler = async () => {
      try {
        // Check if we're in Tauri environment
        if (window.__TAURI__) {
          const { listen } = await import('@tauri-apps/api/event');
          
          // Listen for window close request from Tauri (triggered by system/OS close)
          await listen('window-close-requested', async () => {
            console.log('System window close requested');
            await handleWindowCloseRequest();
          });
        }
      } catch (error) {
        console.error('Failed to setup window close handler:', error);
      }
    };
    
    setupWindowCloseHandler();
    
    // Show window immediately
    setTimeout(async () => {
      if (typeof window !== 'undefined' && window.__TAURI_INTERNALS__) {
        try {
          const { getCurrentWindow } = await import('@tauri-apps/api/window');
          const currentWindow = getCurrentWindow();
          await currentWindow.show();
        } catch (error) {
          console.warn('Failed to show window:', error);
        }
      }
    }, 50);
  })
  
  // Handle window close request with save prompt (for system/OS close events)
  const handleWindowCloseRequest = async () => {
    try {
      // Import scene manager and unsaved changes store
      const { sceneManager } = await import('@/api/scene/SceneManager.js');
      const { unsavedChangesStore } = await import('@/stores/UnsavedChangesStore.jsx');
      const { closeConfirmationActions } = await import('@/stores/CloseConfirmationStore.jsx');
      const { getCurrentProject } = await import('@/api/bridge/projects.js');
      
      // Check if there are any unsaved changes
      if (unsavedChangesStore.hasChanges || sceneManager.hasChanges()) {
        // Show the overlay instead of using browser confirm
        closeConfirmationActions.show({
          projectName: getCurrentProject()?.name || 'Current Project',
          changes: unsavedChangesStore.changes,
          onSaveAndClose: async () => {
            console.log('User chose to save before closing...');
            try {
              const saveResult = await sceneManager.saveScene();
              if (!saveResult.success) {
                alert(`Failed to save: ${saveResult.error}`);
                // Ask if they want to close anyway using browser confirm as fallback
                const closeAnyway = confirm('Save failed. Do you want to close without saving?');
                if (closeAnyway) {
                  await proceedWithClose();
                }
                closeConfirmationActions.hide();
                return;
              }
              console.log('Save successful, proceeding with close');
              await proceedWithClose();
            } catch (error) {
              console.error('Error during save and close:', error);
              closeConfirmationActions.hide();
            }
          },
          onCloseWithoutSaving: async () => {
            console.log('User chose to close without saving');
            await proceedWithClose();
          },
          onClose: () => {
            console.log('User cancelled close');
            closeConfirmationActions.hide();
          }
        });
        return; // Don't proceed with close, wait for user choice
      }
      
      // No unsaved changes, close immediately
      await proceedWithClose();
      
    } catch (error) {
      console.error('Error handling window close request:', error);
      // If there's an error, ask user if they want to close anyway using browser confirm as fallback
      const closeAnyway = confirm('An error occurred while checking for unsaved changes. Do you want to close anyway?');
      if (closeAnyway) {
        await proceedWithClose();
      }
    }
  };

  // Helper function to actually close the application
  const proceedWithClose = async () => {
    console.log('Closing application via system close...');
    try {
      // Emit graceful close event
      const { emit } = await import('@tauri-apps/api/event');
      await emit('proceed-with-close');
      console.log('Graceful close event emitted');
    } catch (closeError) {
      console.error('Failed to emit graceful close event:', closeError);
    }
  };

  return (
    <Engine>
      <Project>
        <KeyboardShortcuts />
        <div class="w-full h-full">
          <Layout />
          <DevNotice />
          <EditorPage />
          
          {/* Render layout components from plugins */}
          <For each={Array.from(layoutComponents().values())}>
            {(layoutComponent) => {
              // Handle both old and new structure for backwards compatibility
              const Component = layoutComponent?.component || layoutComponent;
              return <Component />;
            }}
          </For>
        </div>
        
        {/* Close confirmation overlay */}
        <CloseConfirmationOverlay
          isOpen={() => closeConfirmationStore.isOpen}
          onClose={() => closeConfirmationStore.onClose}
          onSaveAndClose={() => closeConfirmationStore.onSaveAndClose}
          onCloseWithoutSaving={() => closeConfirmationStore.onCloseWithoutSaving}
          projectName={() => closeConfirmationStore.projectName}
          changes={() => closeConfirmationStore.changes}
        />
      </Project>
    </Engine>
  );
}