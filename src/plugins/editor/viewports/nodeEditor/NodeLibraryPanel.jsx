import { For, Show } from 'solid-js';
import { NodeLibrary, NodeTypeColors } from './NodeLibrary';

const NodeLibraryPanel = (props) => {
  const {
    contextMenu,
    activeSubmenu,
    submenuPosition,
    setActiveSubmenu,
    setSubmenuPosition,
    setContextMenu,
    addNode
  } = props;

  const handleNodeClick = (key) => {
    return (e) => {
      e.preventDefault();
      e.stopPropagation();
      console.log('Context menu button clicked for node:', key);
      console.log('Button element:', e.target);
      addNode(key, contextMenu().worldPosition);
      setContextMenu(null);
      setActiveSubmenu(null);
    };
  };

  return (
    <Show when={contextMenu()}>
      <div 
        className="fixed z-50 context-menu"
        style={{
          left: contextMenu().position.x + 'px',
          top: contextMenu().position.y + 'px',
          'pointer-events': 'auto'
        }}
      >
        <div className="relative bg-gray-800 border border-gray-600 rounded-lg shadow-lg py-2 min-w-48 context-menu" style={{ 'pointer-events': 'auto' }}>
          <div className="px-3 py-1 text-gray-300 text-sm font-semibold border-b border-gray-600 mb-1">
            Add Node
          </div>
          
          <div
            className="w-full px-3 py-2 text-left text-gray-200 hover:bg-gray-700 text-sm flex items-center justify-between cursor-pointer relative"
            onMouseEnter={() => {
              setActiveSubmenu('input');
              setSubmenuPosition({ top: 40 });
            }}
          >
            <div className="flex items-center gap-2">
              <div className="w-3 h-3 rounded-full bg-green-500"></div>
              Input
            </div>
            <span className="text-gray-400">›</span>
          </div>

          <div
            className="w-full px-3 py-2 text-left text-gray-200 hover:bg-gray-700 text-sm flex items-center justify-between cursor-pointer relative"
            onMouseEnter={() => {
              setActiveSubmenu('math');
              setSubmenuPosition({ top: 76 });
            }}
          >
            <div className="flex items-center gap-2">
              <div className="w-3 h-3 rounded-full bg-yellow-500"></div>
              Math
            </div>
            <span className="text-gray-400">›</span>
          </div>

          <div
            className="w-full px-3 py-2 text-left text-gray-200 hover:bg-gray-700 text-sm flex items-center justify-between cursor-pointer relative"
            onMouseEnter={() => {
              setActiveSubmenu('output');
              setSubmenuPosition({ top: 112 });
            }}
          >
            <div className="flex items-center gap-2">
              <div className="w-3 h-3 rounded-full bg-red-500"></div>
              Output
            </div>
            <span className="text-gray-400">›</span>
          </div>
        </div>

        <Show when={activeSubmenu()}>
          <div 
            className="absolute bg-gray-800 border border-gray-600 rounded-lg shadow-lg py-2 min-w-48 max-h-64 overflow-y-auto z-10 context-menu"
            style={{
              left: '192px',
              top: submenuPosition().top + 'px',
              'pointer-events': 'auto'
            }}
            onMouseLeave={() => setActiveSubmenu(null)}
          >
            <For each={Object.entries(NodeLibrary).filter(([_, template]) => template.type === activeSubmenu())}>
              {([key, template]) => (
                <button
                  onClick={handleNodeClick(key)}
                  onMouseDown={(e) => {
                    console.log('Mouse down on button:', key);
                  }}
                  className="w-full px-3 py-2 text-left text-gray-200 hover:bg-gray-700 text-sm flex items-center gap-2"
                >
                  <div 
                    className="w-3 h-3 rounded-full flex-shrink-0" 
                    style={{ 'background-color': NodeTypeColors[template.type] }}
                  ></div>
                  <span className="truncate">{template.title}</span>
                </button>
              )}
            </For>
          </div>
        </Show>
      </div>
    </Show>
  );
};

export default NodeLibraryPanel;