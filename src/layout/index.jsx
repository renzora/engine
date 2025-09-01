import { createSignal, onMount, onCleanup, Show } from 'solid-js';
import TopMenu from './topMenu.jsx';
import Toolbar from './Toolbar.jsx';
import Viewport from './viewport.jsx';
import RightPanel from './rightPanel.jsx';
import BottomPanel from './bottomPanel.jsx';
import Footer from './Footer.jsx';
import ModelImporter from '@/pages/editor/AssetLibrary/ModelImporter';
import { horizontalMenuButtonsEnabled, propertiesPanelVisible, bottomPanelVisible, footerVisible } from '@/api/plugin';

const Layout = () => {
  const [showModelImporter, setShowModelImporter] = createSignal(false);

  const handleOpenModelImporter = () => {
    setShowModelImporter(true);
  };

  const handleModelImportComplete = async () => {
    // Trigger asset refresh event for any listening components
    document.dispatchEvent(new CustomEvent('engine:assets-refresh'));
  };

  onMount(() => {
    document.addEventListener('engine:open-model-importer', handleOpenModelImporter);
    
    onCleanup(() => {
      document.removeEventListener('engine:open-model-importer', handleOpenModelImporter);
    });
  });

  return (
    <>
      <div class="fixed inset-0 flex flex-col pointer-events-none z-10" onContextMenu={(e) => e.preventDefault()}>
        <div class="flex-shrink-0 pointer-events-auto z-50">
          <TopMenu />
          <Show when={horizontalMenuButtonsEnabled()}>
            <Toolbar />
          </Show>
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

      <ModelImporter
        isOpen={showModelImporter}
        onClose={() => setShowModelImporter(false)}
        onImportComplete={handleModelImportComplete}
      />
    </>
  );
};

export default Layout;