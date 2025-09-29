import { createSignal, createEffect, onCleanup, For } from 'solid-js';
import { editorStore } from '@/layout/stores/EditorStore';
import { IconAlertTriangle } from '@tabler/icons-solidjs';
import { footerButtons } from '@/api/plugin';

const Footer = () => {
  const [engineInfo, setEngineInfo] = createSignal('Engine Ready');
  const [systemStats, setSystemStats] = createSignal(null);
  
  // Ultra-fast system stats polling (100ms for near real-time)
  createEffect(() => {
    const fetchSystemStats = async () => {
      try {
        const response = await fetch('http://localhost:3001/system/stats');
        if (response.ok) {
          const stats = await response.json();
          setSystemStats(stats);
        }
      } catch (error) {
        console.warn('Failed to fetch system stats:', error);
      }
    };
    
    fetchSystemStats();
    const interval = setInterval(fetchSystemStats, 100); // 100ms = near real-time
    
    onCleanup(() => clearInterval(interval));
  });
  
  // Get reactive values from editor store
  const selection = () => editorStore.selection;
  const selectedEntity = () => selection().entity;
  const transformMode = () => selection().transformMode;
  
  const getSelectionInfo = () => {
    if (selectedEntity()) {
      return `Selected: ${selectedEntity()}`;
    }
    return 'No selection';
  };
  
  const getTransformModeInfo = () => {
    if (transformMode() && transformMode() !== 'select') {
      return ` | ${transformMode().charAt(0).toUpperCase() + transformMode().slice(1)} mode`;
    }
    return '';
  };
  
  const formatBytes = (bytes) => {
    if (!bytes) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  };
  
  const getUsageColor = (usage, type = 'default') => {
    // Different thresholds for different resources
    let yellow, red;
    switch (type) {
      case 'cpu':
        yellow = 70;
        red = 85;
        break;
      case 'memory':
        yellow = 75;
        red = 90;
        break;
      case 'gpu':
        yellow = 80;
        red = 95;
        break;
      default:
        yellow = 70;
        red = 85;
    }
    
    if (usage >= red) return 'text-error'; // Red for critical
    if (usage >= yellow) return 'text-warning'; // Yellow for warning
    return 'text-success'; // Green for good
  };
  
  const needsWarningIcon = (usage, type = 'default') => {
    let red;
    switch (type) {
      case 'cpu':
        red = 85;
        break;
      case 'memory':
        red = 90;
        break;
      case 'gpu':
        red = 95;
        break;
      default:
        red = 85;
    }
    return usage >= red;
  };
  
  const renderSystemStatsInfo = () => {
    const stats = systemStats();
    if (!stats) return <span class="text-base-content/90">Loading...</span>;
    
    const cpu = Math.round(stats.cpu_usage);
    const memory = Math.round(stats.memory_usage);
    const gpu = stats.gpu_usage ? Math.round(stats.gpu_usage) : null;
    
    return (
      <div class="flex items-center gap-2">
        {/* CPU */}
        <div class="flex items-center gap-1">
          <span class={getUsageColor(cpu, 'cpu')}>
            CPU: {cpu}%
          </span>
          {needsWarningIcon(cpu, 'cpu') && (
            <IconAlertTriangle class="w-3 h-3 text-error animate-pulse" />
          )}
        </div>
        
        <span class="text-base-content/30">|</span>
        
        {/* RAM */}
        <div class="flex items-center gap-1">
          <span class={getUsageColor(memory, 'memory')}>
            RAM: {memory}%
          </span>
          {needsWarningIcon(memory, 'memory') && (
            <IconAlertTriangle class="w-3 h-3 text-error animate-pulse" />
          )}
        </div>
        
        {/* GPU */}
        {gpu !== null && (
          <>
            <span class="text-base-content/30">|</span>
            <div class="flex items-center gap-1">
              <span class={getUsageColor(gpu, 'gpu')}>
                GPU: {gpu}%
              </span>
              {needsWarningIcon(gpu, 'gpu') && (
                <IconAlertTriangle class="w-3 h-3 text-error animate-pulse" />
              )}
            </div>
          </>
        )}
      </div>
    );
  };
  
  return (
    <div class="fixed bottom-0 left-0 right-0 h-6 bg-base-200/90 backdrop-blur-md border-t border-base-content/10 text-xs flex items-center justify-between px-3 pointer-events-auto z-50 rounded-t-none">
      {/* Left side - Selection and tool info */}
      <div class="flex items-center gap-4">
        <span class="text-base-content/90">
          {getSelectionInfo()}{getTransformModeInfo()}
        </span>
      </div>
      
      {/* Right side - Status info */}
      <div class="flex items-center gap-4">
        {renderSystemStatsInfo()}
        <span class="text-base-content/90">
          {engineInfo()}
        </span>
        
        {/* Plugin footer buttons */}
        <For each={Array.from(footerButtons().entries())}>
          {([id, button]) => {
            const Component = button.component;
            return Component ? <Component /> : null;
          }}
        </For>
      </div>
    </div>
  );
};

export default Footer;