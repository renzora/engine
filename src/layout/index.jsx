import { createSignal, onMount, onCleanup, Show, createEffect } from 'solid-js';
import TopMenu from './topMenu.jsx';
import Viewport from './viewport.jsx';
import RightPanel from './rightPanel.jsx';
import BottomPanel from './bottomPanel.jsx';
import Footer from './Footer.jsx';
import ModelImporter from '@/pages/editor/AssetLibrary/ModelImporter';
import { ViewportContextMenuProvider } from '@/ui/ViewportContextMenu.jsx';
import KeyboardShortcuts from './KeyboardShortcuts.jsx';
import { editorActions } from './stores/EditorStore.jsx';
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

  const handleOpenMaterialsViewport = async () => {
    // Open the materials viewport
    const { pluginAPI } = await import('@/api/plugin');
    pluginAPI.open('materials', { label: 'Materials' });
  };

  const handleOpenCodeEditor = async (event) => {
    // Open the code editor viewport or switch to existing tab
    const { pluginAPI } = await import('@/api/plugin');
    const { viewportStore, viewportActions } = await import('@/layout/stores/ViewportStore.jsx');
    const { file } = event.detail;
    
    if (!file) {
      // No file specified, just open a new code editor
      pluginAPI.open('code-editor', { 
        label: 'Code Editor'
      });
      return;
    }
    
    // Check if this file is already open in an existing code editor tab
    const existingTab = viewportStore.tabs.find(tab => 
      tab.type === 'code-editor' && 
      tab.initialFile && 
      tab.initialFile.path === file.path
    );
    
    if (existingTab) {
      // File is already open, switch to that tab
      console.log(`[Layout] Switching to existing code editor tab for: ${file.path}`);
      viewportActions.setActiveViewportTab(existingTab.id);
    } else {
      // File is not open, create a new tab
      console.log(`[Layout] Opening new code editor tab for: ${file.path}`);
      const tabName = file.name;
      pluginAPI.open('code-editor', { 
        label: tabName,
        initialFile: file 
      });
    }
  };

  onMount(() => {
    document.addEventListener('engine:open-model-importer', handleOpenModelImporter);
    document.addEventListener('openMaterialsViewport', handleOpenMaterialsViewport);
    document.addEventListener('engine:open-code-editor', handleOpenCodeEditor);
    
    // Listen for global tooltip events
    const handleTooltipShow = (e) => setGlobalTooltip(e.detail);
    const handleTooltipHide = () => setGlobalTooltip(null);
    
    document.addEventListener('global:tooltip-show', handleTooltipShow);
    document.addEventListener('global:tooltip-hide', handleTooltipHide);
    
    onCleanup(() => {
      document.removeEventListener('engine:open-model-importer', handleOpenModelImporter);
      document.removeEventListener('openMaterialsViewport', handleOpenMaterialsViewport);
      document.removeEventListener('engine:open-code-editor', handleOpenCodeEditor);
      document.removeEventListener('global:tooltip-show', handleTooltipShow);
      document.removeEventListener('global:tooltip-hide', handleTooltipHide);
    });
  });

  return (
    <ViewportContextMenuProvider editorActions={editorActions}>
      <KeyboardShortcuts />
      <div class="fixed inset-0 flex flex-col pointer-events-none z-10" onContextMenu={(e) => e.preventDefault()}>
        <div class="flex-shrink-0 pointer-events-auto z-50">
          <TopMenu />
        </div>
      
        <Show when={bottomPanelVisible()}>
          <div class="flex-shrink-0 pointer-events-auto">
            <BottomPanel />
          </div>
        </Show>
        
        <div class="flex-1 relative overflow-hidden pointer-events-auto">
          <Viewport />
          <Show when={propertiesPanelVisible()}>
            <RightPanel />
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
    </ViewportContextMenuProvider>
  );
};

export default Layout;