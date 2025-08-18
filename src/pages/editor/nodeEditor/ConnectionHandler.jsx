import { editorActions } from '@/layout/stores/EditorStore';

const useConnectionHandler = (props) => {
  const { connections, nodes, removeConnectionFromGraph, objectId } = props;

  const getPortPosition = (nodeId, portId, isInput = false) => {
    const node = nodes().find(n => n.id === nodeId);
    if (!node) return { x: 0, y: 0 };

    const nodeX = node.position.x;
    const nodeY = node.position.y;
    const nodeWidth = 200;
    const nodeHeight = 40 + Math.max(
      (node.inputs?.length || 0),
      (node.outputs?.length || 0)
    ) * 25;

    const ports = isInput ? (node.inputs || []) : (node.outputs || []);
    const portIndex = ports.findIndex(p => p.id === portId);
    
    const portY = nodeY + 40 + portIndex * 25 + 12;
    const portX = isInput ? nodeX : nodeX + nodeWidth;

    return { x: portX, y: portY };
  };

  const getConnectionPath = (from, to) => {
    const startX = from.x;
    const startY = from.y;
    const endX = to.x;
    const endY = to.y;

    const midX = startX + (endX - startX) * 0.6;
    
    return `M ${startX} ${startY} C ${midX} ${startY}, ${midX} ${endY}, ${endX} ${endY}`;
  };

  const renderConnections = () => {
    return connections().map(conn => {
      const fromPos = getPortPosition(conn.from.nodeId, conn.from.portId, false);
      const toPos = getPortPosition(conn.to.nodeId, conn.to.portId, true);
      const path = getConnectionPath(fromPos, toPos);

      return (
        <path
          d={path}
          stroke="#6366f1"
          stroke-width={3}
          fill="none"
          style={{ 'pointer-events': 'stroke', cursor: 'pointer' }}
          onClick={() => {        
            removeConnectionFromGraph(objectId, conn.id);
          }}
        />
      );
    });
  };

  return {
    getPortPosition,
    getConnectionPath,
    renderConnections
  };
};

export default useConnectionHandler;
