import { createSignal, onMount, onCleanup, Show, createEffect } from 'solid-js';
import TopMenu from './topMenu.jsx';
import Viewport from './viewport.jsx';
import RightPanel from './rightPanel.jsx';
import BottomPanel from './bottomPanel.jsx';
import Footer from './Footer.jsx';
import ModelImporter from '@/pages/editor/AssetLibrary/ModelImporter';
import { horizontalMenuButtonsEnabled, propertiesPanelVisible, bottomPanelVisible, footerVisible } from '@/api/plugin';

const Layout = () => {
  const [showModelImporter, setShowModelImporter] = createSignal(false);
  const [modelImporterContext, setModelImporterContext] = createSignal(null);
  const [globalTooltip, setGlobalTooltip] = createSignal(null);

  const handleOpenModelImporter = (event) => {
    setModelImporterContext(event.detail || {});
    setShowModelImporter(true);
  };

  const handleModelImportComplete = async () => {
    // Trigger asset refresh event for any listening components
    document.dispatchEvent(new CustomEvent('engine:assets-refresh'));
  };

  onMount(() => {
    document.addEventListener('engine:open-model-importer', handleOpenModelImporter);
    
    // Listen for global tooltip events
    const handleTooltipShow = (e) => setGlobalTooltip(e.detail);
    const handleTooltipHide = () => setGlobalTooltip(null);
    
    document.addEventListener('global:tooltip-show', handleTooltipShow);
    document.addEventListener('global:tooltip-hide', handleTooltipHide);
    
    onCleanup(() => {
      document.removeEventListener('engine:open-model-importer', handleOpenModelImporter);
      document.removeEventListener('global:tooltip-show', handleTooltipShow);
      document.removeEventListener('global:tooltip-hide', handleTooltipHide);
    });
  });

  return (
    <>
      <div class="fixed inset-0 flex flex-col pointer-events-none z-10" onContextMenu={(e) => e.preventDefault()}>
        <div class="flex-shrink-0 pointer-events-auto z-50">
          <TopMenu />
        </div>
      
        <div class="flex-1 relative overflow-hidden pointer-events-auto">
          <Viewport />
          <Show when={propertiesPanelVisible()}>
            <RightPanel />
          </Show>
          <Show when={bottomPanelVisible()}>
            <BottomPanel />
          </Show>
        </div>
        
        <Show when={footerVisible()}>
          <Footer />
        </Show>
      </div>

      {/* Global Tooltip - appears above everything */}
      <Show when={globalTooltip()}>
        <div class="fixed z-[99999] bg-black text-white text-xs p-3 pointer-events-none shadow-xl border border-gray-600 max-w-xs" 
             style={`left: ${globalTooltip().x}px; top: ${globalTooltip().y}px;`}>
          <div class="font-semibold mb-2 text-white truncate">{globalTooltip().asset.name}</div>
          <div class="space-y-1 text-gray-300">
            <div class="truncate">Type: {globalTooltip().asset.extension?.toUpperCase() || 'Unknown'}</div>
            <div class="truncate">Size: {globalTooltip().asset.size ? `${Math.round(globalTooltip().asset.size / 1024)} KB` : 'Unknown'}</div>
            <Show when={globalTooltip().asset.path}>
              <div class="text-gray-400 truncate">Path: {globalTooltip().asset.path}</div>
            </Show>
          </div>
        </div>
      </Show>

      <ModelImporter
        isOpen={showModelImporter}
        onClose={() => setShowModelImporter(false)}
        onImportComplete={handleModelImportComplete}
        context={modelImporterContext}
      />
    </>
  );
};

export default Layout;