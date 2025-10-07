import { createSignal, createEffect, onCleanup, Show } from 'solid-js';
import { IconServer, IconRefresh, IconTrash } from '@tabler/icons-solidjs';
import { useProject } from '@/plugins/splash/ProjectStore';

export default function BridgeModal({ isOpen, onClose }) {
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

  // Handle escape key
  const handleKeyDown = (e) => {
    if (e.key === 'Escape') {
      onClose();
    }
  };

  createEffect(() => {
    if (isOpen()) {
      checkBridgeStatus();
      statusCheckInterval = setInterval(checkBridgeStatus, 5000);
      uptimeInterval = setInterval(() => {
        if (bridgeConnected()) {
          setBridgeInfo(prev => ({ ...prev, uptime: prev.uptime + 1 }));
        }
      }, 1000);
      
      // Add escape key listener
      document.addEventListener('keydown', handleKeyDown);
    } else {
      if (statusCheckInterval) clearInterval(statusCheckInterval);
      if (uptimeInterval) clearInterval(uptimeInterval);
      document.removeEventListener('keydown', handleKeyDown);
    }
  });

  onCleanup(() => {
    if (statusCheckInterval) clearInterval(statusCheckInterval);
    if (uptimeInterval) clearInterval(uptimeInterval);
    document.removeEventListener('keydown', handleKeyDown);
  });

  return (
    <Show when={isOpen()}>
      <div 
        class="fixed inset-0 z-[9999]"
        style={{
          background: 'rgba(0, 0, 0, 0.5)',
          backdropFilter: 'blur(4px)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          padding: '1rem'
        }}
        onClick={onClose}
      >
        <div 
          class="bg-slate-800 rounded-xl border border-slate-700 p-6 w-full max-w-md"
          onClick={(e) => e.stopPropagation()}
        >
          <div class="flex items-center justify-between mb-4">
            <h2 class="text-lg font-bold text-white flex items-center gap-2">
              <IconServer class="w-5 h-5 text-blue-400" />
              Bridge Server Status
            </h2>
            <button
              onClick={onClose}
              class="text-gray-400 hover:text-white transition-colors text-xl"
            >
              ×
            </button>
          </div>

          {/* Connection Status */}
          <div class="mb-4 p-3 bg-slate-700/50 rounded-lg">
            <div class="flex items-center justify-between mb-2">
              <span class="text-gray-300">Connection Status</span>
              <div class="flex items-center gap-2">
                {bridgeConnected() ? (
                  <>
                    <div class="w-2 h-2 bg-green-400 rounded-full animate-pulse"></div>
                    <span class="text-green-400 text-sm">Connected</span>
                  </>
                ) : (
                  <>
                    <div class="w-2 h-2 bg-red-400 rounded-full"></div>
                    <span class="text-red-400 text-sm">Disconnected</span>
                  </>
                )}
              </div>
            </div>
            <div class="text-xs text-gray-500">
              Endpoint: http://localhost:3001
            </div>
          </div>

          {/* Project Info */}
          <Show when={currentProject()?.name}>
            <div class="mb-4 p-3 bg-slate-700/50 rounded-lg">
              <div class="flex items-center justify-between mb-2">
                <span class="text-gray-300">Current Project</span>
              </div>
              <div class="text-sm text-white font-medium mb-1">
                {currentProject()?.name}
              </div>
              <div 
                class="text-xs text-gray-400 cursor-pointer hover:text-gray-300 transition-colors break-all"
                title="Click to copy path"
                onClick={() => navigator.clipboard?.writeText(getProjectPath())}
              >
                {getProjectPath()}
              </div>
            </div>
          </Show>

          {/* Bridge Statistics */}
          <Show when={bridgeConnected()}>
            <div class="mb-4 p-3 bg-slate-700/50 rounded-lg">
              <div class="text-gray-300 mb-2">Server Statistics</div>
              <div class="grid grid-cols-2 gap-2 text-xs">
                <div>
                  <span class="text-gray-400">Uptime:</span>
                  <span class="text-white ml-1">{formatUptime(bridgeInfo().uptime)}</span>
                </div>
                <div>
                  <span class="text-gray-400">Cache Size:</span>
                  <span class="text-white ml-1">{formatBytes(bridgeInfo().cacheSize)}</span>
                </div>
                <div>
                  <span class="text-gray-400">Thumbnails:</span>
                  <span class="text-white ml-1">{bridgeInfo().thumbnailCount}</span>
                </div>
                <div>
                  <span class="text-gray-400">Watched Files:</span>
                  <span class="text-white ml-1">{bridgeInfo().watchedFiles}</span>
                </div>
              </div>
            </div>
          </Show>

          {/* Actions */}
          <div class="flex gap-2">
            <button
              onClick={restartBridge}
              disabled={isRestarting()}
              class="flex-1 flex items-center justify-center gap-2 px-3 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 text-white rounded-lg transition-colors text-sm"
            >
              <Show when={isRestarting()}>
                <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
              </Show>
              <Show when={!isRestarting()}>
                <IconRefresh class="w-4 h-4" />
              </Show>
              {isRestarting() ? 'Restarting...' : 'Restart Bridge'}
            </button>
            
            <Show when={bridgeConnected()}>
              <button
                onClick={clearCache}
                disabled={isClearingCache()}
                class="flex items-center justify-center gap-2 px-3 py-2 bg-red-600 hover:bg-red-700 disabled:bg-gray-600 text-white rounded-lg transition-colors text-sm"
              >
                <Show when={isClearingCache()}>
                  <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
                </Show>
                <Show when={!isClearingCache()}>
                  <IconTrash class="w-4 h-4" />
                </Show>
              </button>
            </Show>
          </div>

          <div class="mt-3 text-xs text-gray-500 text-center">
            Bridge manages project files, thumbnails, and file watching
          </div>
        </div>
      </div>
    </Show>
  );
}