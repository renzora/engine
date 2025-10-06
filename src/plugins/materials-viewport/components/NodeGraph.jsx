import { For, Show } from 'solid-js';
import ConnectionLine from './ConnectionLine.jsx';
import MaterialNode from './MaterialNode.jsx';
import ContextMenu from '@/ui/ContextMenu.jsx';

export default function NodeGraph(props) {
  const {
    nodeGraphRef,
    nodes,
    setNodes,
    connections,
    setConnections,
    selectedNode,
    setSelectedNode,
    draggedNode,
    setDraggedNode,
    draggedNodeTransform,
    setDraggedNodeTransform,
    isDraggingAllNodes,
    setIsDraggingAllNodes,
    isPanning,
    setIsPanning,
    zoom,
    setZoom,
    pan,
    setPan,
    panStart,
    setPanStart,
    allNodesDragStart,
    setAllNodesDragStart,
    draggingConnection,
    setDraggingConnection,
    dragConnectionEnd,
    setDragConnectionEnd,
    hoveredSocket,
    setHoveredSocket,
    contextMenu,
    setContextMenu,
    contextMenuPosition,
    setContextMenuPosition,
    dragOffset,
    setDragOffset,
    handleAssetDrop,
    handleDragOver,
    handleWheel,
    handlePanStart,
    handleContextMenu,
    handleNodeMouseDown,
    handleSocketMouseDown,
    handleSocketMouseEnter,
    handleSocketMouseLeave,
    removeNode,
    removeConnection,
    getSocketScreenPosition,
    createMaterialFromNodes,
    addNodeAtPosition,
    MOVE_THROTTLE_MS
  } = props;

  let lastMoveTime = 0;

  return (
    <div class="flex-1 flex flex-col">
      {/* Node Graph Canvas */}
      <div 
        ref={nodeGraphRef}
        class={`flex-1 relative bg-base-100 h-full ${
          isDraggingAllNodes() ? 'cursor-grabbing' : 
          isPanning() ? 'cursor-grabbing' : 
          'cursor-grab'
        }`}
        style={{
          'background-image': 'radial-gradient(circle, #374151 1px, transparent 1px)',
          'background-size': `${20 * zoom()}px ${20 * zoom()}px`,
          'background-position': `${pan().x}px ${pan().y}px`
        }}
        onDrop={handleAssetDrop}
        onDragOver={handleDragOver}
        onWheel={handleWheel}
        onMouseDown={handlePanStart}
        onContextMenu={handleContextMenu}
      >
        
        {/* Nodes Container */}
        <div 
          class="absolute inset-0"
          style={{
            transform: `translate(${pan().x}px, ${pan().y}px) scale(${zoom()})`,
            'transform-origin': '0 0'
          }}
        >
          {/* Nodes */}
          <For each={nodes()}>
            {(node) => (
              <MaterialNode
                node={node}
                selectedNode={selectedNode}
                draggedNode={draggedNode}
                draggedNodeTransform={draggedNodeTransform}
                handleNodeMouseDown={handleNodeMouseDown}
                handleSocketMouseDown={handleSocketMouseDown}
                handleSocketMouseEnter={handleSocketMouseEnter}
                handleSocketMouseLeave={handleSocketMouseLeave}
                removeNode={removeNode}
                hoveredSocket={hoveredSocket}
                nodes={nodes}
                setNodes={setNodes}
                createMaterialFromNodes={createMaterialFromNodes}
              />
            )}
          </For>
        </div>

        {/* SVG Overlay for Connections */}
        <svg 
          class="absolute inset-0 pointer-events-none" 
          style={{
            width: '100%',
            height: '100%',
            overflow: 'visible'
          }}
        >
          {/* Connection Lines */}
          <For each={connections()}>
            {(connection) => (
              <ConnectionLine
                connection={connection}
                nodes={nodes()}
                getSocketScreenPosition={getSocketScreenPosition}
                zoom={zoom}
                pan={pan}
                draggedNodeId={draggedNode()?.id}
                draggedNodeTransform={draggedNodeTransform}
                onRemove={removeConnection}
              />
            )}
          </For>

          {/* Active connection being dragged */}
          <Show when={draggingConnection()}>
            <path
              d={(() => {
                const drag = draggingConnection();
                if (!drag) return '';
                
                const startPos = getSocketScreenPosition(drag.nodeId, drag.socketId, drag.socketType);
                if (!startPos) return '';
                
                const endPos = dragConnectionEnd();
                const controlOffset = Math.max(80, Math.abs(endPos.x - startPos.x) * 0.4);
                
                if (drag.socketType === 'output') {
                  return `M ${startPos.x} ${startPos.y} C ${startPos.x + controlOffset} ${startPos.y} ${endPos.x - controlOffset} ${endPos.y} ${endPos.x} ${endPos.y}`;
                } else {
                  return `M ${endPos.x} ${endPos.y} C ${endPos.x + controlOffset} ${endPos.y} ${startPos.x - controlOffset} ${startPos.y} ${startPos.x} ${startPos.y}`;
                }
              })()}
              stroke="#3b82f6"
              stroke-width="3"
              fill="none"
              opacity="0.7"
              stroke-dasharray="10,5"
              class="drop-shadow-sm pointer-events-none"
            />
          </Show>
        </svg>

        {/* Context Menu */}
        <Show when={contextMenu()}>
          <ContextMenu
            items={contextMenu()}
            position={contextMenuPosition()}
            onClose={() => {
              setContextMenu(null);
              setContextMenuPosition(null);
            }}
            onSelect={(action) => {
              if (action.type === 'add_node') {
                addNodeAtPosition(action.nodeType, contextMenuPosition());
              }
              setContextMenu(null);
              setContextMenuPosition(null);
            }}
          />
        </Show>
      </div>
    </div>
  );
}