import { createSignal, onMount, onCleanup, Show } from 'solid-js';
import { useProject } from '@/plugins/core/splash/ProjectStore';

export default function BridgeStatus({ onOpenViewport }) {
  const [bridgeConnected, setBridgeConnected] = createSignal(false);
  const [showTooltip, setShowTooltip] = createSignal(false);
  
  const { currentProject } = useProject();
  
  let statusCheckInterval;

  // Check bridge connection status
  const checkBridgeStatus = async () => {
    try {
      const response = await fetch('http://localhost:3001/health');
      setBridgeConnected(response.ok);
    } catch {
      setBridgeConnected(false);
    }
  };

  const getProjectPath = () => {
    const project = currentProject();
    if (!project?.name) return '';
    return `C:\\Users\\james\\solid\\projects\\${project.name}`;
  };

  const handleClick = () => {
    if (onOpenViewport) {
      onOpenViewport();
    }
  };

  onMount(() => {
    checkBridgeStatus();
    statusCheckInterval = setInterval(checkBridgeStatus, 5000);
  });

  onCleanup(() => {
    if (statusCheckInterval) clearInterval(statusCheckInterval);
  });

  return (
    <div class="relative">
      <button
        onClick={handleClick}
        onMouseEnter={() => setShowTooltip(true)}
        onMouseLeave={() => setShowTooltip(false)}
        class="flex items-center gap-1 px-1 py-0.5 text-xs hover:bg-gray-700/50 transition-colors group cursor-pointer"
      >
        <span class="text-gray-300 group-hover:text-white transition-colors">
          {currentProject()?.name || 'No Project'}
        </span>
        <div class={`w-2 h-2 ${bridgeConnected() ? 'bg-green-400' : 'bg-orange-400'}`}></div>
      </button>
      
      <Show when={showTooltip()}>
        <div class="absolute right-full top-1/2 transform -translate-y-1/2 mr-2 px-2 py-1 bg-gray-900/95 text-white text-xs whitespace-nowrap z-[120]">
          <span class={bridgeConnected() ? 'text-green-400' : 'text-yellow-400'}>Bridge {bridgeConnected() ? 'Connected' : 'Disconnected'}</span> {getProjectPath()}
          <div class="absolute left-full top-1/2 transform -translate-y-1/2 border-4 border-transparent border-l-gray-900/95" />
        </div>
      </Show>
    </div>
  );
}