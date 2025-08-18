import { createSignal, onMount, onCleanup, For, Show } from 'solid-js';

const NodeCanvas = (props) => {
  const {
    viewportSize,
    viewTransform,
    nodes,
    selectedNodes,
    tempConnection,
    onMouseDown,
    onContextMenu,
    onWheel,
    renderNode,
    renderConnections,
    getConnectionPath,
    updateNodeGraph,
    objectId,
    setContainerRef
  } = props;

  let containerRef;
  let svgRef;

  onMount(() => {
    const container = containerRef;
    if (!container) return;

    const handleKeyDown = (e) => {
      if (e.key === 'Escape' && props.contextMenu?.()) {
        console.log('Closing context menu - Escape key');
        props.setContextMenu?.(null);
        props.setActiveSubmenu?.(null);
      }
    };

    container.addEventListener('mousedown', onMouseDown);
    container.addEventListener('contextmenu', onContextMenu);
    document.addEventListener('mousemove', props.onMouseMove);
    document.addEventListener('mouseup', props.onMouseUp);
    document.addEventListener('keydown', handleKeyDown);
    container.addEventListener('wheel', onWheel, { passive: false });

    onCleanup(() => {
      container.removeEventListener('mousedown', onMouseDown);
      container.removeEventListener('contextmenu', onContextMenu);
      document.removeEventListener('mousemove', props.onMouseMove);
      document.removeEventListener('mouseup', props.onMouseUp);
      document.removeEventListener('keydown', handleKeyDown);
      container.removeEventListener('wheel', onWheel);
    });
  });

  return (
    <div 
      ref={(el) => {
        containerRef = el;
        setContainerRef?.(el);
      }}
      className="w-full h-full bg-gray-900 overflow-hidden relative"
      style={{ 'user-select': 'none' }}
    >
      <svg
        ref={svgRef}
        width={viewportSize().width}
        height={viewportSize().height}
        className="absolute inset-0"
      >
        <g transform={`translate(${viewTransform().x}, ${viewTransform().y}) scale(${viewTransform().scale})`}>
          <defs>
            <pattern
              id="grid"
              width={50}
              height={50}
              patternUnits="userSpaceOnUse"
            >
              <path
                d="M 50 0 L 0 0 0 50"
                fill="none"
                stroke="#374151"
                stroke-width={1}
                opacity={0.3}
              />
            </pattern>
          </defs>
          <rect
            x={-10000}
            y={-10000}
            width={20000}
            height={20000}
            fill="url(#grid)"
          />

          {renderConnections()}
          <Show when={tempConnection()}>
            <path
              d={getConnectionPath(tempConnection().from, tempConnection().to)}
              stroke="#6366f1"
              stroke-width={3}
              fill="none"
              opacity={0.7}
              stroke-dasharray="5,5"
            />
          </Show>

          <For each={nodes()}>
            {renderNode}
          </For>
        </g>
      </svg>

      <div className="absolute top-4 left-4 flex gap-2">
        <button
          onClick={() => updateNodeGraph(objectId, { viewTransform: { x: 0, y: 0, scale: 1 } })}
          className="px-3 py-1 bg-gray-700 hover:bg-gray-600 text-white rounded text-sm"
        >
          Reset View
        </button>
        <span className="px-3 py-1 bg-gray-800 text-gray-300 rounded text-sm">
          Zoom: {Math.round(viewTransform().scale * 100)}%
        </span>
      </div>

      <div className="absolute top-4 right-4 text-gray-400 text-sm">
        <div>Object: {objectId}</div>
        <div>Nodes: {nodes().length}</div>
        <div>Connections: {props.connections?.().length || 0}</div>
      </div>
    </div>
  );
};

export default NodeCanvas;
