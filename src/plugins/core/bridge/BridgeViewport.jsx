import { createSignal, onMount, onCleanup, Show } from 'solid-js';
import { IconServer, IconRefresh, IconTrash, IconX } from '@tabler/icons-solidjs';
import { useProject } from '@/plugins/splash/ProjectStore';

export default function BridgeViewport({ onClose }) {
  const [bridgeConnected, setBridgeConnected] = createSignal(false);
  const [bridgeInfo, setBridgeInfo] = createSignal({
    status: 'disconnected',
    uptime: 0,
    cacheSize: 0,
    thumbnailCount: 0,
    watchedFiles: 0
  });
  const [isRestarting, setIsRestarting] = createSignal(false);
  const [isClearingCache, setIsClearingCache] = createSignal(false);
  
  const { currentProject } = useProject();
  
  let statusCheckInterval;
  let uptimeInterval;

  // Check bridge connection status
  const checkBridgeStatus = async () => {
    try {
      const response = await fetch('http://localhost:3001/health');
      if (response.ok) {
        const data = await response.json();
        setBridgeConnected(true);
        setBridgeInfo(prev => ({
          ...prev,
          status: 'connected',
          uptime: data.uptime || 0,
          cacheSize: data.cache_size || 0,
          thumbnailCount: data.thumbnail_count || 0,
          watchedFiles: data.watched_files || 0
        }));
      } else {
        setBridgeConnected(false);
        setBridgeInfo(prev => ({ ...prev, status: 'error' }));
      }
    } catch {
      setBridgeConnected(false);
      setBridgeInfo(prev => ({ ...prev, status: 'disconnected' }));
    }
  };

  // Restart bridge server
  const restartBridge = async () => {
    try {
      setIsRestarting(true);
      await fetch('http://localhost:3001/restart', { method: 'POST' });
      
      // Wait a bit for server to restart, then check status
      setTimeout(() => {
        checkBridgeStatus();
        setIsRestarting(false);
      }, 2000);
    } catch (error) {
      console.error('Failed to restart bridge:', error);
      setIsRestarting(false);
    }
  };

  // Clear cache
  const clearCache = async () => {
    try {
      setIsClearingCache(true);
      await fetch('http://localhost:3001/clear-cache', { method: 'POST' });
      await checkBridgeStatus();
    } catch (error) {
      console.error('Failed to clear cache:', error);
    } finally {
      setIsClearingCache(false);
    }
  };

  // Format bytes to human readable
  const formatBytes = (bytes) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
  };

  // Format uptime
  const formatUptime = (seconds) => {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    
    if (days > 0) return `${days}d ${hours}h`;
    if (hours > 0) return `${hours}h ${minutes}m`;
    return `${minutes}m`;
  };

  const getProjectPath = () => {
    const project = currentProject();
    if (!project?.name) return '';
    return `C:\\Users\\james\\solid\\projects\\${project.name}`;
  };

  onMount(() => {
    checkBridgeStatus();
    statusCheckInterval = setInterval(checkBridgeStatus, 5000);
    uptimeInterval = setInterval(() => {
      if (bridgeConnected()) {
        setBridgeInfo(prev => ({ ...prev, uptime: prev.uptime + 1 }));
      }
    }, 1000);
  });

  onCleanup(() => {
    if (statusCheckInterval) clearInterval(statusCheckInterval);
    if (uptimeInterval) clearInterval(uptimeInterval);
  });

  return (
    <div class="w-full h-full bg-slate-900 text-white overflow-auto">
      {/* Header */}
      <div class="flex items-center justify-between p-4 border-b border-slate-700">
        <h1 class="text-xl font-bold flex items-center gap-2">
          <IconServer class="w-6 h-6 text-blue-400" />
          Bridge Server Status
        </h1>
        <button
          onClick={onClose}
          class="p-2 hover:bg-slate-700 rounded-lg transition-colors"
          title="Close Bridge Viewport"
        >
          <IconX class="w-5 h-5" />
        </button>
      </div>

      {/* Content */}
      <div class="p-6 space-y-6">
        {/* Connection Status */}
        <div class="bg-slate-800 rounded-lg p-4">
          <h2 class="text-lg font-semibold mb-3 text-gray-200">Connection Status</h2>
          <div class="flex items-center justify-between mb-3">
            <span class="text-gray-300">Bridge Server</span>
            <div class="flex items-center gap-2">
              {bridgeConnected() ? (
                <>
                  <div class="w-3 h-3 bg-green-400 rounded-full animate-pulse"></div>
                  <span class="text-green-400 font-medium">Connected</span>
                </>
              ) : (
                <>
                  <div class="w-3 h-3 bg-red-400 rounded-full"></div>
                  <span class="text-red-400 font-medium">Disconnected</span>
                </>
              )}
            </div>
          </div>
          <div class="text-sm text-gray-400">
            <strong>Endpoint:</strong> http://localhost:3001
          </div>
        </div>

        {/* Project Information */}
        <Show when={currentProject()?.name}>
          <div class="bg-slate-800 rounded-lg p-4">
            <h2 class="text-lg font-semibold mb-3 text-gray-200">Current Project</h2>
            <div class="space-y-2">
              <div class="text-lg font-medium text-white">
                {currentProject()?.name}
              </div>
              <div 
                class="text-sm text-gray-400 cursor-pointer hover:text-gray-300 transition-colors break-all font-mono bg-slate-700 p-2 rounded"
                title="Click to copy path"
                onClick={() => navigator.clipboard?.writeText(getProjectPath())}
              >
                {getProjectPath()}
              </div>
            </div>
          </div>
        </Show>

        {/* Server Statistics */}
        <Show when={bridgeConnected()}>
          <div class="bg-slate-800 rounded-lg p-4">
            <h2 class="text-lg font-semibold mb-3 text-gray-200">Server Statistics</h2>
            <div class="grid grid-cols-2 gap-4">
              <div class="bg-slate-700 p-3 rounded">
                <div class="text-sm text-gray-400">Uptime</div>
                <div class="text-lg font-semibold text-white">{formatUptime(bridgeInfo().uptime)}</div>
              </div>
              <div class="bg-slate-700 p-3 rounded">
                <div class="text-sm text-gray-400">Cache Size</div>
                <div class="text-lg font-semibold text-white">{formatBytes(bridgeInfo().cacheSize)}</div>
              </div>
              <div class="bg-slate-700 p-3 rounded">
                <div class="text-sm text-gray-400">Thumbnails</div>
                <div class="text-lg font-semibold text-white">{bridgeInfo().thumbnailCount}</div>
              </div>
              <div class="bg-slate-700 p-3 rounded">
                <div class="text-sm text-gray-400">Watched Files</div>
                <div class="text-lg font-semibold text-white">{bridgeInfo().watchedFiles}</div>
              </div>
            </div>
          </div>
        </Show>

        {/* Actions */}
        <div class="bg-slate-800 rounded-lg p-4">
          <h2 class="text-lg font-semibold mb-3 text-gray-200">Server Actions</h2>
          <div class="flex gap-3">
            <button
              onClick={restartBridge}
              disabled={isRestarting()}
              class="flex-1 flex items-center justify-center gap-2 px-4 py-3 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 text-white rounded-lg transition-colors font-medium"
            >
              <Show when={isRestarting()}>
                <div class="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
              </Show>
              <Show when={!isRestarting()}>
                <IconRefresh class="w-5 h-5" />
              </Show>
              {isRestarting() ? 'Restarting...' : 'Restart Bridge'}
            </button>
            
            <Show when={bridgeConnected()}>
              <button
                onClick={clearCache}
                disabled={isClearingCache()}
                class="flex items-center justify-center gap-2 px-4 py-3 bg-red-600 hover:bg-red-700 disabled:bg-gray-600 text-white rounded-lg transition-colors font-medium"
              >
                <Show when={isClearingCache()}>
                  <div class="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
                </Show>
                <Show when={!isClearingCache()}>
                  <IconTrash class="w-5 h-5" />
                </Show>
                Clear Cache
              </button>
            </Show>
          </div>
        </div>

        {/* Help Section */}
        <div class="bg-slate-800 rounded-lg p-4">
          <h2 class="text-lg font-semibold mb-3 text-gray-200">About Bridge Server</h2>
          <div class="text-sm text-gray-400 space-y-2">
            <p>The Bridge Server manages communication between the Renzora Engine frontend and your project files.</p>
            <p><strong>Features:</strong></p>
            <ul class="list-disc list-inside ml-2 space-y-1">
              <li>Project file management and synchronization</li>
              <li>Automatic thumbnail generation for 3D models</li>
              <li>Real-time file watching and change detection</li>
              <li>Asset caching for improved performance</li>
            </ul>
          </div>
        </div>
      </div>
    </div>
  );
}