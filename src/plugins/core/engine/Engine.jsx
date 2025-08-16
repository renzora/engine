import { onMount, onCleanup } from 'solid-js';
import { EngineAPIProvider, engineAPI } from './EngineAPI.jsx';

export default function Engine(props) {
  onMount(async () => {
    console.log('[Engine] Starting Renzora Engine...');
    try {
      await engineAPI.initialize();
      console.log('[Engine] Renzora Engine started successfully!');
    } catch (error) {
      console.error('[Engine] Failed to start Renzora Engine:', error);
    }
  });

  onCleanup(async () => {
    console.log('[Engine] Shutting down Renzora Engine...');
    try {
      await engineAPI.dispose();
      console.log('[Engine] Renzora Engine shut down successfully');
    } catch (error) {
      console.error('[Engine] Error during shutdown:', error);
    }
  });

  return (
    <EngineAPIProvider>
      {props.children}
    </EngineAPIProvider>
  );
}