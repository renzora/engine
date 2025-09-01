import { createSignal, onMount, onCleanup } from 'solid-js';
import { pluginAPI } from '@/api/plugin';

function BridgeFooterButton() {
  const [isOnline, setIsOnline] = createSignal(false);
  const [isChecking, setIsChecking] = createSignal(false);
  let statusInterval;

  const checkBridgeStatus = async () => {
    if (isChecking()) return;
    
    setIsChecking(true);
    try {
      const response = await fetch('http://localhost:3001/health', {
        method: 'GET',
        timeout: 2000
      });
      setIsOnline(response.ok);
    } catch (error) {
      setIsOnline(false);
    } finally {
      setIsChecking(false);
    }
  };

  const openBridgeViewport = () => {
    pluginAPI.createViewportTab('bridge-mgmt', { label: 'Bridge Management' });
  };

  onMount(() => {
    // Check status immediately
    checkBridgeStatus();
    
    // Check status every 5 seconds
    statusInterval = setInterval(checkBridgeStatus, 5000);
  });

  onCleanup(() => {
    if (statusInterval) {
      clearInterval(statusInterval);
    }
  });

  return (
    <button
      onClick={openBridgeViewport}
      className={`flex items-center gap-2 px-3 py-1 rounded text-xs transition-all duration-200 ${
        isOnline() 
          ? 'bg-success/20 text-success hover:bg-success/30' 
          : 'bg-error/20 text-error hover:bg-error/30'
      }`}
      title={`Bridge server is ${isOnline() ? 'online' : 'offline'} - Click to open management`}
    >
      <div className={`w-2 h-2 rounded-full ${
        isChecking() 
          ? 'animate-pulse bg-warning' 
          : isOnline() 
            ? 'bg-success' 
            : 'bg-error'
      }`} />
      <span>Bridge</span>
      <span className={`text-xs ${isOnline() ? 'text-success/70' : 'text-error/70'}`}>
        {isOnline() ? 'Online' : 'Offline'}
      </span>
    </button>
  );
}

export default BridgeFooterButton;