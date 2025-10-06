import { createMemo } from 'solid-js';

export default function ConnectionLine(props) {
  const pathData = createMemo(() => {
    const { connection, nodes, getSocketScreenPosition, zoom, pan, draggedNodeId, draggedNodeTransform } = props;
    
    // Find the actual nodes to ensure we have current positions
    const fromNode = nodes.find(n => n.id === connection.from.nodeId);
    const toNode = nodes.find(n => n.id === connection.to.nodeId);
    
    if (!fromNode || !toNode) return '';
    
    // Access signals to make this reactive
    const currentZoom = zoom();
    const currentPan = pan();
    const currentDragTransform = draggedNodeTransform(); // Make reactive to drag transform
    
    // Check if this connection involves the dragged node
    const isDraggedConnection = draggedNodeId === connection.from.nodeId || draggedNodeId === connection.to.nodeId;
    
    // Get socket positions in screen coordinates
    const fromPos = getSocketScreenPosition(connection.from.nodeId, connection.from.socketId, 'output');
    const toPos = getSocketScreenPosition(connection.to.nodeId, connection.to.socketId, 'input');
    
    if (!fromPos || !toPos) return '';
    
    const fromX = fromPos.x;
    const fromY = fromPos.y;
    const toX = toPos.x;
    const toY = toPos.y;
    
    // Create curved connection with better control points
    const controlOffset = Math.max(80, Math.abs(toX - fromX) * 0.4);
    return `M ${fromX} ${fromY} C ${fromX + controlOffset} ${fromY} ${toX - controlOffset} ${toY} ${toX} ${toY}`;
  });
  
  return (
    <g>
      {/* Invisible thicker path for easier clicking */}
      <path
        d={pathData()}
        stroke="transparent"
        stroke-width="12"
        fill="none"
        class="cursor-pointer"
        style={{'pointer-events': 'all'}}
        onClick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          console.log('Connection clicked, removing:', props.connection.id);
          props.onRemove?.(props.connection.id);
        }}
      />
      {/* Visible connection line */}
      <path
        d={pathData()}
        stroke="#3b82f6"
        stroke-width="3"
        fill="none"
        class="drop-shadow-sm pointer-events-none"
      />
    </g>
  );
}