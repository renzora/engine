import { createSignal } from 'solid-js';

const useDragDropHandler = (props) => {
  const {
    containerRef,
    viewTransform,
    nodes,
    selectedNodes,
    setSelectedNodes,
    setTempConnection,
    updateNodeGraph,
    addConnectionAndGenerateProperties,
    objectId
  } = props;

  const [dragState, setDragState] = createSignal({
    isDragging: false,
    dragType: null,
    dragData: null,
    startPos: { x: 0, y: 0 },
    offset: { x: 0, y: 0 }
  });

  const screenToWorld = (screenX, screenY) => {
    const rect = containerRef?.getBoundingClientRect();
    if (!rect) return { x: screenX, y: screenY };
    
    const x = (screenX - rect.left - viewTransform().x) / viewTransform().scale;
    const y = (screenY - rect.top - viewTransform().y) / viewTransform().scale;
    return { x, y };
  };

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

  const handleMouseDown = (e) => {
    if (e.target.closest('.context-menu')) {
      console.log('Click on context menu element, ignoring');
      return;
    }
    
    if (props.contextMenu?.()) {
      console.log('Closing context menu - clicked outside');
      props.setContextMenu?.(null);
      props.setActiveSubmenu?.(null);
      return;
    }
    
    const target = e.target;
    const worldPos = screenToWorld(e.clientX, e.clientY);

    if (target.classList.contains('node-port')) {
      const nodeId = target.dataset.nodeId;
      const portId = target.dataset.portId;
      const isInput = target.dataset.isInput === 'true';
      
      setDragState({
        isDragging: true,
        dragType: 'cable',
        dragData: { nodeId, portId, isInput },
        startPos: worldPos
      });

      const portPos = getPortPosition(nodeId, portId, isInput);
      setTempConnection({
        from: isInput ? worldPos : portPos,
        to: isInput ? portPos : worldPos,
        isReverse: isInput
      });
      return;
    }

    const nodeElement = target.closest('.node');
    if (nodeElement) {
      const nodeId = nodeElement.dataset.nodeId;
      const node = nodes().find(n => n.id === nodeId);
      
      if (node) {
        const offset = {
          x: worldPos.x - node.position.x,
          y: worldPos.y - node.position.y
        };

        setDragState({
          isDragging: true,
          dragType: 'node',
          dragData: { nodeId },
          startPos: worldPos,
          offset
        });

        if (!e.ctrlKey && !e.metaKey) {
          setSelectedNodes(new Set([nodeId]));
        } else {
          setSelectedNodes(prev => {
            const newSet = new Set(prev);
            if (newSet.has(nodeId)) {
              newSet.delete(nodeId);
            } else {
              newSet.add(nodeId);
            }
            return newSet;
          });
        }
      }
      return;
    }

    setDragState({
      isDragging: true,
      dragType: 'pan',
      startPos: { x: e.clientX, y: e.clientY },
      offset: viewTransform()
    });
  };

  const handleMouseMove = (e) => {
    const currentDragState = dragState();
    if (!currentDragState.isDragging) return;

    if (currentDragState.dragType === 'node' && currentDragState.dragData) {
      const worldPos = screenToWorld(e.clientX, e.clientY);
      const newX = worldPos.x - currentDragState.offset.x;
      const newY = worldPos.y - currentDragState.offset.y;
      const updatedNodes = nodes().map(node => 
        node.id === currentDragState.dragData.nodeId
          ? { ...node, position: { x: newX, y: newY } }
          : node
      );
      updateNodeGraph(objectId, { nodes: updatedNodes });
    } else if (currentDragState.dragType === 'cable') {
      const worldPos = screenToWorld(e.clientX, e.clientY);
      setTempConnection(prev => {
        if (!prev) return null;
        return {
          ...prev,
          [prev.isReverse ? 'from' : 'to']: worldPos
        };
      });
    } else if (currentDragState.dragType === 'pan') {
      const deltaX = e.clientX - currentDragState.startPos.x;
      const deltaY = e.clientY - currentDragState.startPos.y;
      
      const newTransform = {
        ...currentDragState.offset,
        x: currentDragState.offset.x + deltaX,
        y: currentDragState.offset.y + deltaY
      };
      updateNodeGraph(objectId, { viewTransform: newTransform });
    }
  };

  const handleMouseUp = (e) => {
    const currentDragState = dragState();
    
    if (currentDragState.dragType === 'cable') {
      const target = e.target;
      if (target.classList.contains('node-port')) {
        const targetNodeId = target.dataset.nodeId;
        const targetPortId = target.dataset.portId;
        const targetIsInput = target.dataset.isInput === 'true';
        const source = currentDragState.dragData;
        
        if (source.isInput !== targetIsInput) {
          const fromNode = source.isInput ? targetNodeId : source.nodeId;
          const fromPort = source.isInput ? targetPortId : source.portId;
          const toNode = source.isInput ? source.nodeId : targetNodeId;
          const toPort = source.isInput ? source.portId : targetPortId;
          const newConnection = {
            id: `conn-${Date.now()}`,
            from: { nodeId: fromNode, portId: fromPort },
            to: { nodeId: toNode, portId: toPort }
          };
          
          addConnectionAndGenerateProperties(objectId, newConnection);
        }
      }
      setTempConnection(null);
    }

    setDragState({
      isDragging: false,
      dragType: null,
      dragData: null
    });
  };

  const handleWheel = (e) => {
    e.preventDefault();
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    const newScale = Math.max(0.1, Math.min(3, viewTransform().scale * delta));
    
    const rect = containerRef?.getBoundingClientRect();
    if (!rect) return;
    
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;
    
    const worldMouseX = (mouseX - viewTransform().x) / viewTransform().scale;
    const worldMouseY = (mouseY - viewTransform().y) / viewTransform().scale;
    
    const newX = mouseX - worldMouseX * newScale;
    const newY = mouseY - worldMouseY * newScale;
    
    const newTransform = {
      x: newX,
      y: newY,
      scale: newScale
    };
    updateNodeGraph(objectId, { viewTransform: newTransform });
  };

  return {
    dragState,
    screenToWorld,
    getPortPosition,
    handleMouseDown,
    handleMouseMove,
    handleMouseUp,
    handleWheel
  };
};

export default useDragDropHandler;