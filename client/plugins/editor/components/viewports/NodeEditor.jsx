import { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import { useSnapshot } from 'valtio';
import { globalStore, actions, babylonScene } from '@/store.js';
import { NodeLibrary, PortTypeColors, NodeTypeColors } from './NodeLibrary.jsx';

// Node Editor Component with Cable Connections
const NodeEditor = ({ tab, objectId }) => {
  const { nodeEditor } = useSnapshot(globalStore.editor);
  const { 
    getNodeGraph, setNodeGraph, updateNodeGraph, addConnectionAndGenerateProperties, removeConnectionFromGraph,
    addPropertySection, bindNodeToProperty, initializeObjectProperties 
  } = actions.editor;
  
  const svgRef = useRef(null);
  const containerRef = useRef(null);
  const dragStateRef = useRef({
    isDragging: false,
    dragType: null, // 'node' | 'cable'
    dragData: null,
    startPos: { x: 0, y: 0 },
    offset: { x: 0, y: 0 }
  });

  // Get object name from Babylon scene
  const getObjectName = useCallback(() => {
    const scene = babylonScene?.current;
    if (!scene) return objectId;

    const allObjects = [
      ...(scene.meshes || []),
      ...(scene.transformNodes || []),
      ...(scene.lights || []),
      ...(scene.cameras || [])
    ];
    
    const babylonObject = allObjects.find(obj => 
      (obj.uniqueId || obj.name) === objectId
    );
    
    return babylonObject ? (babylonObject.name || objectId) : objectId;
  }, [objectId]);

  // Initialize or get existing node graph
  const initializeGraph = useCallback(() => {
    const existingGraph = getNodeGraph(objectId);
    if (existingGraph) {
      return existingGraph;
    }

    // Create empty graph - no default nodes
    const defaultGraph = {
      nodes: [],
      connections: [],
      viewTransform: { x: 0, y: 0, scale: 1 }
    };

    setNodeGraph(objectId, defaultGraph);
    
    // Initialize basic object properties (but empty - no sections until output nodes are connected)
    initializeObjectProperties(objectId);
    
    return defaultGraph;
  }, [objectId, getNodeGraph, setNodeGraph]);

  // Get current graph data
  const currentGraph = useMemo(() => {
    return nodeEditor.graphs[objectId] || initializeGraph();
  }, [nodeEditor.graphs, objectId, initializeGraph]);

  const nodes = currentGraph.nodes || [];
  const connections = currentGraph.connections || [];
  const viewTransform = currentGraph.viewTransform || { x: 0, y: 0, scale: 1 };
  const [selectedNodes, setSelectedNodes] = useState(new Set());
  const [tempConnection, setTempConnection] = useState(null);
  const [contextMenu, setContextMenu] = useState(null);
  const [activeSubmenu, setActiveSubmenu] = useState(null);
  const [submenuPosition, setSubmenuPosition] = useState({ top: 0 });

  // Get viewport dimensions
  const [viewportSize, setViewportSize] = useState({ width: 800, height: 600 });

  useEffect(() => {
    const updateSize = () => {
      if (containerRef.current) {
        const rect = containerRef.current.getBoundingClientRect();
        setViewportSize({ width: rect.width, height: rect.height });
      }
    };

    updateSize();
    window.addEventListener('resize', updateSize);
    return () => window.removeEventListener('resize', updateSize);
  }, []);

  // Convert screen coordinates to node editor coordinates
  const screenToWorld = useCallback((screenX, screenY) => {
    const rect = containerRef.current?.getBoundingClientRect();
    if (!rect) return { x: screenX, y: screenY };
    
    const x = (screenX - rect.left - viewTransform.x) / viewTransform.scale;
    const y = (screenY - rect.top - viewTransform.y) / viewTransform.scale;
    return { x, y };
  }, [viewTransform]);

  // Get port position in world coordinates
  const getPortPosition = useCallback((nodeId, portId, isInput = false) => {
    const node = nodes.find(n => n.id === nodeId);
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
  }, [nodes]);

  // Generate SVG path for connection
  const getConnectionPath = useCallback((from, to) => {
    const startX = from.x;
    const startY = from.y;
    const endX = to.x;
    const endY = to.y;

    const midX = startX + (endX - startX) * 0.6;
    
    return `M ${startX} ${startY} C ${midX} ${startY}, ${midX} ${endY}, ${endX} ${endY}`;
  }, []);

  // Add new node at position
  const addNode = useCallback((nodeType, position) => {
    console.log('Adding node:', nodeType, 'at position:', position); // Debug log
    
    const template = NodeLibrary[nodeType];
    if (!template) {
      console.error('Template not found for node type:', nodeType);
      return;
    }

    const newNode = {
      id: `${nodeType}-${Date.now()}`,
      type: template.type,
      title: template.title,
      position: position,
      inputs: template.inputs ? template.inputs.map(input => ({ ...input, id: `${input.id}-${Date.now()}` })) : undefined,
      outputs: template.outputs ? template.outputs.map(output => ({ ...output, id: `${output.id}-${Date.now()}` })) : undefined
    };

    console.log('Created new node:', newNode); // Debug log

    const updatedNodes = [...nodes, newNode];
    console.log('Updated nodes array:', updatedNodes); // Debug log
    
    updateNodeGraph(objectId, { nodes: updatedNodes });

    // Initialize object properties if not already done (but don't auto-create property sections)
    initializeObjectProperties(objectId);

  }, [nodes, objectId, updateNodeGraph, initializeObjectProperties, addPropertySection]);

  // Handle right click for context menu
  const handleContextMenu = useCallback((e) => {
    e.preventDefault();
    const worldPos = screenToWorld(e.clientX, e.clientY);
    
    // Get container bounds to calculate relative position
    const rect = containerRef.current?.getBoundingClientRect();
    if (!rect) return;
    
    const menuData = {
      position: { 
        x: e.clientX - rect.left, 
        y: e.clientY - rect.top 
      },
      worldPosition: worldPos
    };
    
    console.log('Setting context menu:', menuData); // Debug log
    setContextMenu(menuData);
  }, [screenToWorld]);

  // Handle mouse events
  const handleMouseDown = useCallback((e) => {
    // Don't handle clicks on context menu elements
    if (e.target.closest('.context-menu')) {
      console.log('Click on context menu element, ignoring');
      return;
    }
    
    // Close context menu on any click outside
    if (contextMenu) {
      console.log('Closing context menu - clicked outside');
      setContextMenu(null);
      setActiveSubmenu(null);
      return; // Don't process other mouse events when closing menu
    }
    
    const target = e.target;
    const worldPos = screenToWorld(e.clientX, e.clientY);

    // Check if clicking on a port
    if (target.classList.contains('node-port')) {
      const nodeId = target.dataset.nodeId;
      const portId = target.dataset.portId;
      const isInput = target.dataset.isInput === 'true';
      
      dragStateRef.current = {
        isDragging: true,
        dragType: 'cable',
        dragData: { nodeId, portId, isInput },
        startPos: worldPos
      };

      const portPos = getPortPosition(nodeId, portId, isInput);
      setTempConnection({
        from: isInput ? worldPos : portPos,
        to: isInput ? portPos : worldPos,
        isReverse: isInput
      });
      return;
    }

    // Check if clicking on a node
    const nodeElement = target.closest('.node');
    if (nodeElement) {
      const nodeId = nodeElement.dataset.nodeId;
      const node = nodes.find(n => n.id === nodeId);
      
      if (node) {
        const offset = {
          x: worldPos.x - node.position.x,
          y: worldPos.y - node.position.y
        };

        dragStateRef.current = {
          isDragging: true,
          dragType: 'node',
          dragData: { nodeId },
          startPos: worldPos,
          offset
        };

        // Handle node selection
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

    // Background click - start pan
    dragStateRef.current = {
      isDragging: true,
      dragType: 'pan',
      startPos: { x: e.clientX, y: e.clientY },
      offset: viewTransform
    };
  }, [nodes, screenToWorld, getPortPosition, viewTransform]);

  const handleMouseMove = useCallback((e) => {
    const dragState = dragStateRef.current;
    if (!dragState.isDragging) return;

    if (dragState.dragType === 'node' && dragState.dragData) {
      const worldPos = screenToWorld(e.clientX, e.clientY);
      const newX = worldPos.x - dragState.offset.x;
      const newY = worldPos.y - dragState.offset.y;

      // Update node position in store
      const updatedNodes = nodes.map(node => 
        node.id === dragState.dragData.nodeId
          ? { ...node, position: { x: newX, y: newY } }
          : node
      );
      updateNodeGraph(objectId, { nodes: updatedNodes });
    } else if (dragState.dragType === 'cable') {
      const worldPos = screenToWorld(e.clientX, e.clientY);
      setTempConnection(prev => {
        if (!prev) return null;
        return {
          ...prev,
          [prev.isReverse ? 'from' : 'to']: worldPos
        };
      });
    } else if (dragState.dragType === 'pan') {
      const deltaX = e.clientX - dragState.startPos.x;
      const deltaY = e.clientY - dragState.startPos.y;
      
      const newTransform = {
        ...dragState.offset,
        x: dragState.offset.x + deltaX,
        y: dragState.offset.y + deltaY
      };
      updateNodeGraph(objectId, { viewTransform: newTransform });
    }
  }, [screenToWorld, nodes, objectId, updateNodeGraph]);

  const handleMouseUp = useCallback((e) => {
    const dragState = dragStateRef.current;
    
    if (dragState.dragType === 'cable') {
      const target = e.target;
      if (target.classList.contains('node-port')) {
        const targetNodeId = target.dataset.nodeId;
        const targetPortId = target.dataset.portId;
        const targetIsInput = target.dataset.isInput === 'true';
        
        const source = dragState.dragData;
        
        // Validate connection (output to input only)
        if (source.isInput !== targetIsInput) {
          const fromNode = source.isInput ? targetNodeId : source.nodeId;
          const fromPort = source.isInput ? targetPortId : source.portId;
          const toNode = source.isInput ? source.nodeId : targetNodeId;
          const toPort = source.isInput ? source.portId : targetPortId;
          
          // Add new connection via store action
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

    dragStateRef.current = {
      isDragging: false,
      dragType: null,
      dragData: null
    };
  }, []);

  // Handle wheel for zoom
  const handleWheel = useCallback((e) => {
    e.preventDefault();
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    const newScale = Math.max(0.1, Math.min(3, viewTransform.scale * delta));
    
    const rect = containerRef.current?.getBoundingClientRect();
    if (!rect) return;
    
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;
    
    const worldMouseX = (mouseX - viewTransform.x) / viewTransform.scale;
    const worldMouseY = (mouseY - viewTransform.y) / viewTransform.scale;
    
    const newX = mouseX - worldMouseX * newScale;
    const newY = mouseY - worldMouseY * newScale;
    
    const newTransform = {
      x: newX,
      y: newY,
      scale: newScale
    };
    updateNodeGraph(objectId, { viewTransform: newTransform });
  }, [viewTransform, objectId, updateNodeGraph]);

  // Handle escape key to close context menu
  const handleKeyDown = useCallback((e) => {
    if (e.key === 'Escape' && contextMenu) {
      console.log('Closing context menu - Escape key');
      setContextMenu(null);
      setActiveSubmenu(null);
    }
  }, [contextMenu]);

  // Setup event listeners
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    container.addEventListener('mousedown', handleMouseDown);
    container.addEventListener('contextmenu', handleContextMenu);
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    document.addEventListener('keydown', handleKeyDown);
    container.addEventListener('wheel', handleWheel, { passive: false });

    return () => {
      container.removeEventListener('mousedown', handleMouseDown);
      container.removeEventListener('contextmenu', handleContextMenu);
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.removeEventListener('keydown', handleKeyDown);
      container.removeEventListener('wheel', handleWheel);
    };
  }, [handleMouseDown, handleMouseMove, handleMouseUp, handleWheel, handleContextMenu, handleKeyDown]);

  // Render node component
  const renderNode = (node) => {
    const isSelected = selectedNodes.has(node.id);
    const nodeWidth = 200;
    const inputCount = node.inputs?.length || 0;
    const outputCount = node.outputs?.length || 0;
    const nodeHeight = 40 + Math.max(inputCount, outputCount) * 25;
    const nodeTypeColor = NodeTypeColors[node.type] || '#6b7280';

    return (
      <g key={node.id}>
        {/* Node Body */}
        <rect
          className="node"
          data-node-id={node.id}
          x={node.position.x}
          y={node.position.y}
          width={nodeWidth}
          height={nodeHeight}
          rx={8}
          fill={isSelected ? "#374151" : "#1f2937"}
          stroke={isSelected ? "#3b82f6" : nodeTypeColor}
          strokeWidth={isSelected ? 2 : 2}
          style={{ cursor: 'move' }}
        />
        
        {/* Node Title Bar */}
        <rect
          x={node.position.x}
          y={node.position.y}
          width={nodeWidth}
          height={30}
          rx={8}
          fill={nodeTypeColor}
          style={{ pointerEvents: 'none' }}
        />
        <rect
          x={node.position.x}
          y={node.position.y + 15}
          width={nodeWidth}
          height={15}
          fill={nodeTypeColor}
          style={{ pointerEvents: 'none' }}
        />
        
        {/* Node Title */}
        <text
          x={node.position.x + nodeWidth / 2}
          y={node.position.y + 20}
          textAnchor="middle"
          fill="#ffffff"
          fontSize="13"
          fontWeight="bold"
          pointerEvents="none"
        >
          {node.title}
        </text>

        {/* Close/Delete Button (show for all nodes now) */}
        <g>
          <circle
            cx={node.position.x + nodeWidth - 12}
            cy={node.position.y + 12}
            r={8}
            fill="#ef4444"
            stroke="#ffffff"
            strokeWidth={1}
            className="node-close-button"
            style={{ cursor: 'pointer' }}
            onClick={(e) => {
              e.stopPropagation();
              console.log('Deleting node:', node.id);
              actions.editor.deleteNodeAndCleanupProperties(objectId, node.id);
            }}
          />
          <text
            x={node.position.x + nodeWidth - 12}
            y={node.position.y + 16}
            textAnchor="middle"
            fill="#ffffff"
            fontSize="11"
            fontWeight="bold"
            pointerEvents="none"
          >
            ×
          </text>
        </g>

        {/* Input Ports */}
        {node.inputs?.map((input, index) => {
          const portColor = PortTypeColors[input.type] || '#6b7280';
          return (
            <g key={input.id}>
              <circle
                className="node-port"
                data-node-id={node.id}
                data-port-id={input.id}
                data-is-input="true"
                cx={node.position.x}
                cy={node.position.y + 40 + index * 25 + 12}
                r={6}
                fill={portColor}
                stroke="#ffffff"
                strokeWidth={2}
                style={{ cursor: 'crosshair' }}
              />
              <text
                x={node.position.x + 15}
                y={node.position.y + 40 + index * 25 + 17}
                fill="#d1d5db"
                fontSize="12"
                pointerEvents="none"
              >
                {input.name}
              </text>
            </g>
          );
        })}

        {/* Output Ports */}
        {node.outputs?.map((output, index) => {
          const portColor = PortTypeColors[output.type] || '#6b7280';
          return (
            <g key={output.id}>
              <circle
                className="node-port"
                data-node-id={node.id}
                data-port-id={output.id}
                data-is-input="false"
                cx={node.position.x + nodeWidth}
                cy={node.position.y + 40 + index * 25 + 12}
                r={6}
                fill={portColor}
                stroke="#ffffff"
                strokeWidth={2}
                style={{ cursor: 'crosshair' }}
              />
              <text
                x={node.position.x + nodeWidth - 15}
                y={node.position.y + 40 + index * 25 + 17}
                textAnchor="end"
                fill="#d1d5db"
                fontSize="12"
                pointerEvents="none"
              >
                {output.name}
              </text>
            </g>
          );
        })}
      </g>
    );
  };

  // Render connections
  const renderConnections = () => {
    return connections.map(conn => {
      const fromPos = getPortPosition(conn.from.nodeId, conn.from.portId, false);
      const toPos = getPortPosition(conn.to.nodeId, conn.to.portId, true);
      const path = getConnectionPath(fromPos, toPos);

      return (
        <path
          key={conn.id}
          d={path}
          stroke="#6366f1"
          strokeWidth={3}
          fill="none"
          style={{ pointerEvents: 'stroke', cursor: 'pointer' }}
          onClick={() => {
            // Remove connection on click
            removeConnectionFromGraph(objectId, conn.id);
          }}
        />
      );
    });
  };

  return (
    <div 
      ref={containerRef}
      className="w-full h-full bg-gray-900 overflow-hidden relative"
      style={{ userSelect: 'none' }}
    >
      <svg
        ref={svgRef}
        width={viewportSize.width}
        height={viewportSize.height}
        className="absolute inset-0"
      >
        <g transform={`translate(${viewTransform.x}, ${viewTransform.y}) scale(${viewTransform.scale})`}>
          {/* Grid */}
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
                strokeWidth={1}
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

          {/* Connections */}
          {renderConnections()}

          {/* Temporary connection while dragging */}
          {tempConnection && (
            <path
              d={getConnectionPath(tempConnection.from, tempConnection.to)}
              stroke="#6366f1"
              strokeWidth={3}
              fill="none"
              opacity={0.7}
              strokeDasharray="5,5"
            />
          )}

          {/* Nodes */}
          {nodes.map(renderNode)}
        </g>
      </svg>

      {/* UI Overlay */}
      <div className="absolute top-4 left-4 flex gap-2">
        <button
          onClick={() => updateNodeGraph(objectId, { viewTransform: { x: 0, y: 0, scale: 1 } })}
          className="px-3 py-1 bg-gray-700 hover:bg-gray-600 text-white rounded text-sm"
        >
          Reset View
        </button>
        <span className="px-3 py-1 bg-gray-800 text-gray-300 rounded text-sm">
          Zoom: {Math.round(viewTransform.scale * 100)}%
        </span>
      </div>

      <div className="absolute top-4 right-4 text-gray-400 text-sm">
        <div>Object: {objectId}</div>
        <div>Nodes: {nodes.length}</div>
        <div>Connections: {connections.length}</div>
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <div 
          className="fixed z-50 context-menu"
          style={{
            left: contextMenu.position.x,
            top: contextMenu.position.y,
            pointerEvents: 'auto'
          }}
        >
          {/* Main Menu */}
          <div className="relative bg-gray-800 border border-gray-600 rounded-lg shadow-lg py-2 min-w-48 context-menu" style={{ pointerEvents: 'auto' }}>
            <div className="px-3 py-1 text-gray-300 text-sm font-semibold border-b border-gray-600 mb-1">
              Add Node
            </div>
            
            {/* Category Menu Items - Simplified */}
            <div
              className="w-full px-3 py-2 text-left text-gray-200 hover:bg-gray-700 text-sm flex items-center justify-between cursor-pointer relative"
              onMouseEnter={() => {
                setActiveSubmenu('input');
                setSubmenuPosition({ top: 40 }); // Header height + this item's position
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
                setSubmenuPosition({ top: 76 }); // Header + first item height
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
                setSubmenuPosition({ top: 112 }); // Header + 2 items
              }}
            >
              <div className="flex items-center gap-2">
                <div className="w-3 h-3 rounded-full bg-red-500"></div>
                Output
              </div>
              <span className="text-gray-400">›</span>
            </div>
          </div>

          {/* Submenu - positioned outside the main menu */}
          {activeSubmenu && (
            <div 
              className="absolute bg-gray-800 border border-gray-600 rounded-lg shadow-lg py-2 min-w-48 max-h-64 overflow-y-auto z-10 context-menu"
              style={{
                left: 192, // Width of main menu
                top: submenuPosition.top,
                pointerEvents: 'auto'
              }}
              onMouseLeave={() => setActiveSubmenu(null)}
            >
              {Object.entries(NodeLibrary)
                .filter(([_, template]) => template.type === activeSubmenu)
                .map(([key, template]) => (
                  <button
                    key={key}
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      console.log('Context menu button clicked for node:', key); // Debug log
                      console.log('Button element:', e.target);
                      addNode(key, contextMenu.worldPosition);
                      setContextMenu(null);
                      setActiveSubmenu(null);
                    }}
                    onMouseDown={(e) => {
                      console.log('Mouse down on button:', key);
                    }}
                    className="w-full px-3 py-2 text-left text-gray-200 hover:bg-gray-700 text-sm flex items-center gap-2"
                  >
                    <div 
                      className="w-3 h-3 rounded-full flex-shrink-0" 
                      style={{ backgroundColor: NodeTypeColors[template.type] }}
                    ></div>
                    <span className="truncate">{template.title}</span>
                  </button>
                ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default NodeEditor;