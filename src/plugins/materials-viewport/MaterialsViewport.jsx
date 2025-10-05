import { createSignal, createEffect, onMount, onCleanup, Show, For, createMemo } from 'solid-js';
import { renderStore } from '@/render/store';
import { 
  IconPalette, 
  IconSphere,
  IconBox,
  IconCube,
  IconSettings,
  IconPhoto,
  IconCircleDot,
  IconMinus,
  IconPlus
} from '@tabler/icons-solidjs';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial.js';
import { PBRMaterial } from '@babylonjs/core/Materials/PBR/pbrMaterial.js';
import { Color3 } from '@babylonjs/core/Maths/math.color.js';
import { CreateSphere } from '@babylonjs/core/Meshes/Builders/sphereBuilder.js';
import { CreateBox } from '@babylonjs/core/Meshes/Builders/boxBuilder.js';
import { CreateGround } from '@babylonjs/core/Meshes/Builders/groundBuilder.js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';
import { FreeCamera } from '@babylonjs/core/Cameras/freeCamera.js';
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight.js';
import { Scene } from '@babylonjs/core/scene.js';
import { Engine } from '@babylonjs/core/Engines/engine.js';
import { Texture } from '@babylonjs/core/Materials/Textures/texture.js';

// Texture Preview Component - Prevents image reloading during drag
function TexturePreview(props) {
  const imageSrc = createMemo(() => `/api/assets/thumbnail/${props.asset.id}`);
  const imageAlt = createMemo(() => props.asset.name);
  
  return (
    <div class="relative overflow-hidden rounded border border-base-300 bg-base-200 h-16 mb-2">
      <img 
        src={imageSrc()}
        alt={imageAlt()}
        class="w-full h-full object-cover"
        onError={(e) => {
          e.target.style.display = 'none';
          e.target.nextElementSibling.style.display = 'flex';
        }}
      />
      <div class="absolute inset-0 bg-base-300 flex items-center justify-center text-base-content/40 text-xs hidden">
        No Preview
      </div>
      <div class="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/80 to-transparent px-1 py-0.5">
        <div class="text-white text-xs truncate">{imageAlt()}</div>
      </div>
    </div>
  );
}

// Connection Line Component - Reactive to node drag transforms
function ConnectionLine(props) {
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
    <path
      d={pathData()}
      stroke="#3b82f6"
      stroke-width="3"
      fill="none"
      class="drop-shadow-sm"
    />
  );
}

export default function MaterialsViewport() {
  const [previewShape, setPreviewShape] = createSignal('sphere');
  const [nodes, setNodes] = createSignal([]);
  const [connections, setConnections] = createSignal([]);
  const [draggedNode, setDraggedNode] = createSignal(null);
  const [dragOffset, setDragOffset] = createSignal({ x: 0, y: 0 });
  const [selectedNode, setSelectedNode] = createSignal(null);
  const [draggedNodeTransform, setDraggedNodeTransform] = createSignal({ x: 0, y: 0 });
  const [currentMaterial, setCurrentMaterial] = createSignal(null);
  const [zoom, setZoom] = createSignal(1);
  const [pan, setPan] = createSignal({ x: 0, y: 0 });
  const [isPanning, setIsPanning] = createSignal(false);
  const [panStart, setPanStart] = createSignal({ x: 0, y: 0 });
  const [draggingConnection, setDraggingConnection] = createSignal(null);
  const [dragConnectionEnd, setDragConnectionEnd] = createSignal({ x: 0, y: 0 });
  const [hoveredSocket, setHoveredSocket] = createSignal(null);
  
  // Throttle mouse move updates for better performance
  let lastMoveTime = 0;
  const MOVE_THROTTLE_MS = 16; // ~60fps
  
  // Preview scene refs
  let previewCanvasRef;
  let previewScene;
  let previewEngine;
  let previewMesh;
  let nodeGraphRef;
  
  // Node types
  const NODE_TYPES = {
    MATERIAL_OUTPUT: 'MaterialOutput',
    TEXTURE_SAMPLE: 'TextureSample',
    CONSTANT: 'Constant',
    MULTIPLY: 'Multiply',
    ADD: 'Add',
    LERP: 'Lerp'
  };

  // Initialize preview scene
  const initPreviewScene = () => {
    if (!previewCanvasRef) return;
    
    previewEngine = new Engine(previewCanvasRef, true);
    previewScene = new Scene(previewEngine);
    previewScene.clearColor = new Color3(0.15, 0.15, 0.15);
    
    // Setup camera
    const camera = new FreeCamera('previewCamera', new Vector3(0, 0, -4), previewScene);
    camera.setTarget(Vector3.Zero());
    camera.attachControl(previewCanvasRef, true);
    
    // Setup lighting
    const light = new HemisphericLight('previewLight', new Vector3(0, 1, 0), previewScene);
    light.intensity = 0.8;
    
    // Create initial preview mesh
    updatePreviewMesh();
    
    // Start render loop
    previewEngine.runRenderLoop(() => {
      previewScene.render();
    });
    
    // Handle resize
    window.addEventListener('resize', () => {
      previewEngine.resize();
    });
  };

  // Update preview mesh based on selected shape
  const updatePreviewMesh = () => {
    if (!previewScene) return;
    
    // Dispose existing mesh
    if (previewMesh) {
      previewMesh.dispose();
    }
    
    // Create new mesh based on shape
    switch (previewShape()) {
      case 'sphere':
        previewMesh = CreateSphere('previewSphere', { diameter: 2 }, previewScene);
        break;
      case 'cube':
        previewMesh = CreateBox('previewCube', { size: 2 }, previewScene);
        break;
      case 'plane':
        previewMesh = CreateGround('previewPlane', { width: 2, height: 2 }, previewScene);
        break;
      default:
        previewMesh = CreateSphere('previewSphere', { diameter: 2 }, previewScene);
    }
    
    // Apply current material if available
    if (currentMaterial()) {
      previewMesh.material = currentMaterial();
    }
  };

  // Initialize with default nodes
  const initializeDefaultNodes = () => {
    // Calculate center position based on viewport
    const centerX = nodeGraphRef ? (nodeGraphRef.clientWidth / 2) - 100 : 400; // Offset by half node width
    const centerY = nodeGraphRef ? (nodeGraphRef.clientHeight / 2) - 100 : 300; // Offset by half node height
    
    const outputNode = {
      id: 'material-output',
      type: NODE_TYPES.MATERIAL_OUTPUT,
      position: { x: centerX, y: centerY },
      title: 'Material Output',
      inputs: [
        { id: 'baseColor', name: 'Base Color', type: 'color', value: new Color3(0.8, 0.8, 0.8) },
        { id: 'roughness', name: 'Roughness', type: 'float', value: 0.5 },
        { id: 'metallic', name: 'Metallic', type: 'float', value: 0.0 },
        { id: 'normal', name: 'Normal', type: 'vector', value: null },
        { id: 'emissive', name: 'Emissive', type: 'color', value: new Color3(0, 0, 0) }
      ],
      outputs: []
    };
    
    setNodes([outputNode]);
    createMaterialFromNodes();
  };

  // Create material from node graph
  const createMaterialFromNodes = () => {
    const scene = previewScene;
    if (!scene) return;
    
    // Find material output node
    const outputNode = nodes().find(n => n.type === NODE_TYPES.MATERIAL_OUTPUT);
    if (!outputNode) return;
    
    // Create PBR material
    const material = new PBRMaterial('NodeMaterial', scene);
    
    // Apply values from output node
    const baseColorInput = outputNode.inputs.find(i => i.id === 'baseColor');
    const roughnessInput = outputNode.inputs.find(i => i.id === 'roughness');
    const metallicInput = outputNode.inputs.find(i => i.id === 'metallic');
    const emissiveInput = outputNode.inputs.find(i => i.id === 'emissive');
    
    if (baseColorInput?.value) {
      material.baseColor = baseColorInput.value;
    }
    if (roughnessInput?.value !== undefined) {
      material.roughness = roughnessInput.value;
    }
    if (metallicInput?.value !== undefined) {
      material.metallic = metallicInput.value;
    }
    if (emissiveInput?.value) {
      material.emissiveColor = emissiveInput.value;
    }
    
    setCurrentMaterial(material);
    
    // Apply to preview mesh
    if (previewMesh) {
      previewMesh.material = material;
    }
  };

  // Add new node
  const addNode = (type, position, asset = null) => {
    const nodeId = `node-${Date.now()}`;
    let newNode;
    
    switch (type) {
      case NODE_TYPES.TEXTURE_SAMPLE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: asset ? asset.name || 'Texture Sample' : 'Texture Sample',
          asset: asset, // Store the asset data for preview
          inputs: [
            { id: 'texture', name: 'Texture', type: 'texture', value: asset },
            { id: 'uv', name: 'UV', type: 'vector2', value: null }
          ],
          outputs: [
            { id: 'rgb', name: 'RGB', type: 'color' },
            { id: 'r', name: 'R', type: 'float' },
            { id: 'g', name: 'G', type: 'float' },
            { id: 'b', name: 'B', type: 'float' },
            { id: 'a', name: 'A', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.CONSTANT:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Constant',
          inputs: [
            { id: 'value', name: 'Value', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.MULTIPLY:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Multiply',
          inputs: [
            { id: 'a', name: 'A', type: 'float', value: 1.0 },
            { id: 'b', name: 'B', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
    }
    
    if (newNode) {
      setNodes(prev => [...prev, newNode]);
    }
  };

  // Handle node drag
  const handleNodeMouseDown = (e, node) => {
    e.preventDefault();
    e.stopPropagation();
    
    const rect = nodeGraphRef.getBoundingClientRect();
    // Convert screen coordinates to node graph coordinates
    const screenX = e.clientX - rect.left;
    const screenY = e.clientY - rect.top;
    const graphX = (screenX - pan().x) / zoom();
    const graphY = (screenY - pan().y) / zoom();
    
    setDraggedNode(node);
    setDragOffset({
      x: graphX - node.position.x,
      y: graphY - node.position.y
    });
    
    // Initialize drag transform with current position to prevent jump
    setDraggedNodeTransform({ x: node.position.x, y: node.position.y });
    
    setSelectedNode(node);
  };

  const handleMouseMove = (e) => {
    if (draggedNode()) {
      const rect = nodeGraphRef.getBoundingClientRect();
      const newX = (e.clientX - rect.left - pan().x) / zoom() - dragOffset().x;
      const newY = (e.clientY - rect.top - pan().y) / zoom() - dragOffset().y;
      
      // Just update the transform, don't re-render the entire node
      setDraggedNodeTransform({ x: newX, y: newY });
    } else if (isPanning()) {
      handlePanMove(e);
    } else if (draggingConnection()) {
      const rect = nodeGraphRef.getBoundingClientRect();
      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;
      
      // Store screen coordinates for SVG rendering
      setDragConnectionEnd({ x: screenX, y: screenY });
    }
  };

  const handleMouseUp = () => {
    const wasDragging = !!draggedNode();
    
    // Commit the final position to the nodes signal when drag ends
    if (wasDragging) {
      const transform = draggedNodeTransform();
      setNodes(prev => prev.map(node => 
        node.id === draggedNode().id 
          ? { ...node, position: { x: transform.x, y: transform.y } }
          : node
      ));
      setSocketPositionCache(new Map()); // Clear cache after position change
    }
    
    setDraggedNode(null);
    setIsPanning(false);
    
    // Handle connection drop
    if (draggingConnection()) {
      const hovered = hoveredSocket();
      if (hovered && canConnect(draggingConnection(), hovered)) {
        addConnection(draggingConnection(), hovered);
      }
      setDraggingConnection(null);
      setHoveredSocket(null);
    }
  };

  // Connection management
  const canConnect = (from, to) => {
    // Can't connect to same node
    if (from.nodeId === to.nodeId) return false;
    
    // Can only connect output to input
    if (from.type !== 'output' || to.type !== 'input') return false;
    
    // Check if connection already exists
    return !connections().some(conn => 
      conn.from.nodeId === from.nodeId && conn.from.socketId === from.socketId &&
      conn.to.nodeId === to.nodeId && conn.to.socketId === to.socketId
    );
  };

  const addConnection = (from, to) => {
    const newConnection = {
      id: `conn-${Date.now()}`,
      from,
      to
    };
    setConnections(prev => [...prev, newConnection]);
    createMaterialFromNodes();
  };

  const removeConnection = (connectionId) => {
    setConnections(prev => prev.filter(conn => conn.id !== connectionId));
    createMaterialFromNodes();
  };

  // Cached socket positions to reduce DOM queries during drag
  const [socketPositionCache, setSocketPositionCache] = createSignal(new Map());
  
  // Calculate socket position in screen coordinates (for SVG that's not transformed)
  const getSocketScreenPosition = (nodeId, socketId, socketType) => {
    const cacheKey = `${nodeId}-${socketId}-${socketType}`;
    const cache = socketPositionCache();
    
    // For dragged nodes, calculate position based on transform
    const isDraggedNodeSocket = draggedNode()?.id === nodeId;
    
    if (isDraggedNodeSocket) {
      // Get the socket element to use the same calculation method but with transform position
      const socketElement = document.querySelector(`[data-socket="${cacheKey}"]`);
      if (!socketElement || !nodeGraphRef) return null;
      
      const node = nodes().find(n => n.id === nodeId);
      if (!node) return null;
      
      const transform = draggedNodeTransform();
      
      // Get the current DOM position to calculate offset
      const socketRect = socketElement.getBoundingClientRect();
      const graphRect = nodeGraphRef.getBoundingClientRect();
      
      // Calculate how much the socket is offset from the node's top-left
      const nodeElement = socketElement.closest('.absolute');
      if (!nodeElement) return null;
      
      const nodeRect = nodeElement.getBoundingClientRect();
      const socketOffsetX = (socketRect.left + socketRect.width / 2) - nodeRect.left;
      const socketOffsetY = (socketRect.top + socketRect.height / 2) - nodeRect.top;
      
      // Apply the offset to the transform position
      const nodeScreenX = transform.x * zoom() + pan().x;
      const nodeScreenY = transform.y * zoom() + pan().y;
      
      const screenX = nodeScreenX + socketOffsetX;
      const screenY = nodeScreenY + socketOffsetY;
      
      return { x: screenX, y: screenY };
    }
    
    // Use cached position if available and not dragging this node
    if (cache.has(cacheKey)) {
      return cache.get(cacheKey);
    }
    
    const socketElement = document.querySelector(`[data-socket="${cacheKey}"]`);
    
    if (socketElement && nodeGraphRef) {
      const socketRect = socketElement.getBoundingClientRect();
      const graphRect = nodeGraphRef.getBoundingClientRect();
      
      // Calculate center of socket in screen coordinates relative to graph container
      const socketCenterX = socketRect.left + socketRect.width / 2 - graphRect.left;
      const socketCenterY = socketRect.top + socketRect.height / 2 - graphRect.top;
      
      const position = { x: socketCenterX, y: socketCenterY };
      
      // Cache the position
      const newCache = new Map(cache);
      newCache.set(cacheKey, position);
      setSocketPositionCache(newCache);
      
      return position;
    }
    
    return null;
  };

  // Calculate socket position in graph coordinates (for transformed elements)
  const getSocketPosition = (nodeId, socketId, socketType) => {
    // Try to get actual screen position first
    const screenPos = getSocketScreenPosition(nodeId, socketId, socketType);
    if (screenPos) {
      // Convert screen position to graph coordinates
      const graphX = (screenPos.x - pan().x) / zoom();
      const graphY = (screenPos.y - pan().y) / zoom();
      return { x: graphX, y: graphY };
    }
    
    // Fallback to calculated position if DOM element not found
    const node = nodes().find(n => n.id === nodeId);
    if (!node) return { x: 0, y: 0 };
    
    const nodeWidth = 200; // Approximate node width
    const nodeHeaderHeight = 48; // Header height
    const socketSpacing = 28; // Space between sockets
    const socketStartY = nodeHeaderHeight + 24; // Starting Y position for sockets
    
    let socketIndex = 0;
    
    if (socketType === 'input' && node.inputs) {
      socketIndex = node.inputs.findIndex(s => s.id === socketId);
    } else if (socketType === 'output' && node.outputs) {
      socketIndex = node.outputs.findIndex(s => s.id === socketId);
    }
    
    if (socketIndex === -1) return { x: 0, y: 0 };
    
    // Calculate socket position relative to node
    const socketX = socketType === 'input' ? node.position.x : node.position.x + nodeWidth;
    const socketY = node.position.y + socketStartY + (socketIndex * socketSpacing);
    
    return { x: socketX, y: socketY };
  };

  // Socket event handlers
  const handleSocketMouseDown = (e, nodeId, socket, socketType) => {
    e.preventDefault();
    e.stopPropagation();
    
    console.log('Socket clicked:', socketType, nodeId, socket.id);
    
    if (socketType === 'output') {
      console.log('Starting connection drag from output socket');
      
      // Get the socket position for logging
      const socketPos = getSocketPosition(nodeId, socket.id, socketType);
      console.log('Starting drag from socket position:', socketPos);
      
      setDraggingConnection({
        nodeId,
        socketId: socket.id,
        type: 'output'
      });
      
      const rect = nodeGraphRef.getBoundingClientRect();
      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;
      
      // Store screen coordinates for SVG rendering
      setDragConnectionEnd({ x: screenX, y: screenY });
    }
  };

  const handleSocketMouseEnter = (nodeId, socket, socketType) => {
    if (draggingConnection()) {
      setHoveredSocket({
        nodeId,
        socketId: socket.id,
        type: socketType
      });
    }
  };

  const handleSocketMouseLeave = () => {
    setHoveredSocket(null);
  };

  // Handle zoom
  const handleWheel = (e) => {
    e.preventDefault();
    e.stopPropagation();
    
    const rect = nodeGraphRef.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;
    
    const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
    const newZoom = Math.max(0.1, Math.min(3, zoom() * zoomFactor));
    
    // Zoom towards mouse position
    const currentPan = pan();
    const newPan = {
      x: currentPan.x - (mouseX / zoom() - mouseX / newZoom),
      y: currentPan.y - (mouseY / zoom() - mouseY / newZoom)
    };
    
    setZoom(newZoom);
    setPan(newPan);
  };

  // Handle pan start
  const handlePanStart = (e) => {
    if (e.button === 1 || (e.button === 0 && e.shiftKey)) { // Middle mouse or Shift+Left mouse
      e.preventDefault();
      e.stopPropagation();
      setIsPanning(true);
      setPanStart({ x: e.clientX, y: e.clientY });
    }
  };

  // Handle pan move
  const handlePanMove = (e) => {
    if (isPanning()) {
      e.preventDefault();
      const deltaX = e.clientX - panStart().x;
      const deltaY = e.clientY - panStart().y;
      
      setPan(prev => ({
        x: prev.x + deltaX / zoom(),
        y: prev.y + deltaY / zoom()
      }));
      
      setPanStart({ x: e.clientX, y: e.clientY });
    }
  };

  // Reset zoom and pan
  const resetView = () => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  };

  // Zoom to fit all nodes
  const zoomToFit = () => {
    const nodesList = nodes();
    if (nodesList.length === 0) return;
    
    const padding = 100;
    const minX = Math.min(...nodesList.map(n => n.position.x)) - padding;
    const maxX = Math.max(...nodesList.map(n => n.position.x + 200)) + padding; // Assume node width ~200
    const minY = Math.min(...nodesList.map(n => n.position.y)) - padding;
    const maxY = Math.max(...nodesList.map(n => n.position.y + 150)) + padding; // Assume node height ~150
    
    const width = maxX - minX;
    const height = maxY - minY;
    
    if (nodeGraphRef) {
      const rect = nodeGraphRef.getBoundingClientRect();
      const scaleX = rect.width / width;
      const scaleY = rect.height / height;
      const newZoom = Math.min(scaleX, scaleY, 1);
      
      const centerX = (minX + maxX) / 2;
      const centerY = (minY + maxY) / 2;
      
      setZoom(newZoom);
      setPan({
        x: rect.width / 2 - centerX * newZoom,
        y: rect.height / 2 - centerY * newZoom
      });
    }
  };

  // Handle asset drop
  const handleAssetDrop = (e) => {
    e.preventDefault();
    e.stopPropagation();
    
    // Try different data transfer formats
    let dragData = null;
    try {
      dragData = JSON.parse(e.dataTransfer.getData('application/json'));
    } catch {
      try {
        dragData = JSON.parse(e.dataTransfer.getData('text/plain'));
      } catch {
        try {
          dragData = JSON.parse(e.dataTransfer.getData('application/x-asset-drag'));
        } catch {
          // Try global drag data as fallback
          dragData = window._currentDragData;
        }
      }
    }
    
    if (!dragData) return;
    
    // Handle single asset or multiple assets
    const assets = dragData.assets || [dragData];
    
    for (const asset of assets) {
      // Check if it's an image asset
      const isImage = asset.category === 'images' || 
                     asset.extension?.match(/\.(jpg|jpeg|png|tiff|bmp|webp|gif)$/i) ||
                     asset.mimeType?.startsWith('image/');
      
      if (isImage) {
        const rect = nodeGraphRef.getBoundingClientRect();
        const position = {
          x: e.clientX - rect.left - 100,
          y: e.clientY - rect.top - 50
        };
        
        // Create texture sample node with asset data
        addNode(NODE_TYPES.TEXTURE_SAMPLE, position, asset);
        
        console.log('Created texture node for:', asset.name);
        break; // Only create one node for now
      }
    }
  };

  // Handle drag over
  const handleDragOver = (e) => {
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = 'copy';
  };

  // Effects
  createEffect(() => {
    updatePreviewMesh();
  });

  createEffect(() => {
    createMaterialFromNodes();
  });

  // Clear socket cache when zoom or pan changes
  createEffect(() => {
    zoom(); // Access zoom signal to make this reactive
    pan(); // Access pan signal to make this reactive
    setSocketPositionCache(new Map()); // Clear cache on zoom/pan changes
  });

  onMount(() => {
    setTimeout(() => {
      initPreviewScene();
      initializeDefaultNodes();
    }, 100);
    
    // Add global mouse event listeners
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  });

  onCleanup(() => {
    document.removeEventListener('mousemove', handleMouseMove);
    document.removeEventListener('mouseup', handleMouseUp);
    
    if (previewEngine) {
      previewEngine.dispose();
    }
  });

  return (
    <div class="h-full flex bg-base-100">
      {/* Left Panel - Preview */}
      <div class="w-96 border-r border-base-300 flex flex-col bg-base-200">
        {/* Preview Controls */}
        <div class="p-4 border-b border-base-300">
          <h3 class="text-md font-semibold mb-3">Material Preview</h3>
          
          {/* Preview Shape Selector */}
          <div class="flex gap-2 mb-3">
            <button
              class={`btn btn-sm ${previewShape() === 'sphere' ? 'btn-primary' : 'btn-ghost'}`}
              onClick={() => setPreviewShape('sphere')}
              title="Sphere"
            >
              <IconSphere class="w-4 h-4" />
            </button>
            <button
              class={`btn btn-sm ${previewShape() === 'cube' ? 'btn-primary' : 'btn-ghost'}`}
              onClick={() => setPreviewShape('cube')}
              title="Cube"
            >
              <IconCube class="w-4 h-4" />
            </button>
            <button
              class={`btn btn-sm ${previewShape() === 'plane' ? 'btn-primary' : 'btn-ghost'}`}
              onClick={() => setPreviewShape('plane')}
              title="Plane"
            >
              <IconBox class="w-4 h-4" />
            </button>
          </div>
        </div>
        
        {/* Preview Canvas */}
        <div class="h-64 bg-base-300 relative">
          <canvas
            ref={previewCanvasRef}
            class="w-full h-full"
            style={{ display: 'block' }}
          />
        </div>
        
        {/* Material Info */}
        <div class="p-4 border-t border-base-300">
          <div class="text-sm text-base-content/60">
            <div class="flex items-center gap-2 mb-2">
              <IconPalette class="w-4 h-4" />
              <span class="font-medium">Node Material</span>
            </div>
            <div class="text-xs">
              Nodes: {nodes().length} | Connections: {connections().length}
            </div>
          </div>
        </div>
      </div>
      
      {/* Right Panel - Node Graph */}
      <div class="flex-1 flex flex-col">
        {/* Node Graph Header */}
        <div class="p-4 border-b border-base-300 bg-base-200">
          <div class="flex items-center justify-between">
            <h3 class="text-md font-semibold">Material Graph</h3>
            <div class="flex gap-2">
              {/* Zoom Controls */}
              <div class="flex items-center gap-1 bg-base-100 rounded px-2 py-1 border border-base-300">
                <button 
                  class="btn btn-xs btn-ghost"
                  onClick={() => setZoom(prev => Math.max(0.1, prev * 0.8))}
                  title="Zoom Out"
                >
                  <IconMinus class="w-3 h-3" />
                </button>
                <span class="text-xs font-mono w-12 text-center">{Math.round(zoom() * 100)}%</span>
                <button 
                  class="btn btn-xs btn-ghost"
                  onClick={() => setZoom(prev => Math.min(3, prev * 1.25))}
                  title="Zoom In"
                >
                  <IconPlus class="w-3 h-3" />
                </button>
                <div class="divider divider-horizontal mx-1"></div>
                <button 
                  class="btn btn-xs btn-ghost"
                  onClick={resetView}
                  title="Reset View"
                >
                  <IconSettings class="w-3 h-3" />
                </button>
                <button 
                  class="btn btn-xs btn-ghost"
                  onClick={zoomToFit}
                  title="Zoom to Fit"
                >
                  <IconSphere class="w-3 h-3" />
                </button>
              </div>
              
              <div class="dropdown dropdown-end">
                <button class="btn btn-sm btn-primary">
                  <IconPlus class="w-4 h-4" />
                  Add Node
                </button>
                <ul class="dropdown-content menu p-2 shadow bg-base-100 rounded-box w-52 border border-base-300">
                  <li>
                    <a onClick={() => addNode(NODE_TYPES.TEXTURE_SAMPLE, { x: 400, y: 200 })}>
                      <IconPhoto class="w-4 h-4" />
                      Texture Sample
                    </a>
                  </li>
                  <li>
                    <a onClick={() => addNode(NODE_TYPES.CONSTANT, { x: 200, y: 300 })}>
                      <IconCircleDot class="w-4 h-4" />
                      Constant
                    </a>
                  </li>
                  <li>
                    <a onClick={() => addNode(NODE_TYPES.MULTIPLY, { x: 300, y: 400 })}>
                      <IconSettings class="w-4 h-4" />
                      Multiply
                    </a>
                  </li>
                </ul>
              </div>
            </div>
          </div>
        </div>
        
        {/* Node Graph Canvas */}
        <div 
          ref={nodeGraphRef}
          class="flex-1 relative bg-base-100 cursor-grab"
          style={{
            'background-image': 'radial-gradient(circle, #374151 1px, transparent 1px)',
            'background-size': `${20 * zoom()}px ${20 * zoom()}px`,
            'background-position': `${pan().x}px ${pan().y}px`
          }}
          onDrop={handleAssetDrop}
          onDragOver={handleDragOver}
          onWheel={handleWheel}
          onMouseDown={handlePanStart}
          onContextMenu={(e) => e.preventDefault()}
        >
          {/* Drop Zone Indicator */}
          <div class="absolute inset-4 border-2 border-dashed border-base-300 rounded-lg flex items-center justify-center pointer-events-none">
            <div class="text-center text-base-content/40">
              <IconPhoto class="w-12 h-12 mx-auto mb-2" />
              <p class="text-lg font-medium mb-1">Drag textures from Asset Library</p>
              <p class="text-sm">Or use the Add Node menu to create material nodes</p>
            </div>
          </div>
          
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
              {(node) => {
                const isDragged = () => draggedNode()?.id === node.id;
                const position = () => isDragged() ? draggedNodeTransform() : node.position;
                
                return (
                  <div
                    class={`absolute bg-base-200 border shadow-lg min-w-44 rounded-lg overflow-hidden ${
                      selectedNode() === node ? 'border-primary ring-2 ring-primary/30' : 'border-base-300 hover:border-base-content/20'
                    } ${isDragged() ? '' : 'transition-all duration-200'}`}
                    style={{
                      left: `${position().x}px`,
                      top: `${position().y}px`,
                      'z-index': isDragged() ? '1000' : 'auto'
                    }}
                  >
                {/* Node Header - Compact like Unreal */}
                <div 
                  class={`px-3 py-1.5 text-sm font-medium cursor-grab select-none transition-all flex items-center gap-2 ${
                    selectedNode() === node ? 'text-primary bg-primary/20' : 'text-base-content/80 hover:text-base-content bg-base-300 hover:bg-base-300/80'
                  }`}
                  onMouseDown={(e) => handleNodeMouseDown(e, node)}
                >
                  {/* Type indicator */}
                  <div class={`w-2 h-2 rounded-sm ${
                    node.type === 'MaterialOutput' ? 'bg-success' :
                    node.type === 'TextureSample' ? 'bg-info' :
                    node.type === 'Constant' ? 'bg-warning' :
                    'bg-neutral'
                  }`}></div>
                  <span class="truncate">{node.title}</span>
                  
                  {/* Active indicator line like tabs */}
                  <Show when={selectedNode() === node}>
                    <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-primary"></div>
                  </Show>
                </div>
                
                {/* Node Body - Compact like Unreal */}
                <div class="bg-base-100 p-2 space-y-1">
                  {/* Texture Preview - Compact */}
                  <Show when={node.type === NODE_TYPES.TEXTURE_SAMPLE && node.asset}>
                    <TexturePreview asset={node.asset} />
                  </Show>
                  
                  {/* Inputs - Compact */}
                  <Show when={node.inputs?.length > 0}>
                    <For each={node.inputs}>
                      {(input) => (
                        <div class="flex items-center justify-between py-0.5 group">
                          <div class="flex items-center gap-1.5 flex-1 min-w-0">
                            <div 
                              class={`w-2.5 h-2.5 rounded-full border cursor-pointer transition-all duration-200 pointer-events-auto flex-shrink-0 ${
                                hoveredSocket()?.nodeId === node.id && hoveredSocket()?.socketId === input.id
                                  ? 'bg-primary border-primary scale-110'
                                  : 'bg-base-300 border-base-400 hover:border-primary hover:bg-primary/20'
                              }`}
                              style={{ position: 'relative', 'z-index': '10' }}
                              data-socket={`${node.id}-${input.id}-input`}
                              onMouseDown={(e) => handleSocketMouseDown(e, node.id, input, 'input')}
                              onMouseEnter={() => handleSocketMouseEnter(node.id, input, 'input')}
                              onMouseLeave={handleSocketMouseLeave}
                              onClick={(e) => {
                                e.stopPropagation();
                              }}
                            ></div>
                            <span class="text-xs text-base-content/80 truncate">{input.name}</span>
                          </div>
                          <Show when={input.type === 'float' && input.value !== null}>
                            <input
                              type="number"
                              class="input input-xs w-14 text-right text-xs border-base-300 bg-base-200"
                              value={input.value}
                              step="0.1"
                              onChange={(e) => {
                                const newNodes = nodes().map(n => 
                                  n.id === node.id 
                                    ? {
                                        ...n,
                                        inputs: n.inputs.map(i =>
                                          i.id === input.id 
                                            ? { ...i, value: parseFloat(e.target.value) }
                                            : i
                                        )
                                      }
                                    : n
                                );
                                setNodes(newNodes);
                              }}
                            />
                          </Show>
                        </div>
                      )}
                    </For>
                  </Show>
                  
                  {/* Outputs - Compact */}
                  <Show when={node.outputs?.length > 0}>
                    <For each={node.outputs}>
                      {(output) => (
                        <div class="flex items-center justify-between py-0.5 group">
                          <div class="flex items-center gap-1.5 flex-1 min-w-0">
                            <span class="text-xs text-base-content/80 truncate">{output.name}</span>
                          </div>
                          <div 
                            class={`w-3 h-3 rounded-full border cursor-pointer transition-all duration-200 pointer-events-auto flex-shrink-0 ${
                              hoveredSocket()?.nodeId === node.id && hoveredSocket()?.socketId === output.id
                                ? 'bg-primary border-primary scale-125 shadow-lg shadow-primary/50'
                                : 'bg-gradient-to-br from-primary to-primary-focus border-primary hover:border-primary-focus hover:scale-110'
                            }`}
                            style={{ position: 'relative', 'z-index': '10' }}
                            data-socket={`${node.id}-${output.id}-output`}
                            onMouseDown={(e) => handleSocketMouseDown(e, node.id, output, 'output')}
                            onMouseEnter={() => handleSocketMouseEnter(node.id, output, 'output')}
                            onMouseLeave={handleSocketMouseLeave}
                            onClick={(e) => {
                              e.stopPropagation();
                            }}
                          ></div>
                        </div>
                      )}
                    </For>
                  </Show>
                </div>
              </div>
                );
              }}
            </For>
          </div>
          
          {/* Debug Info */}
          <div class="absolute top-2 left-2 bg-black/80 text-white p-2 rounded text-xs font-mono pointer-events-none">
            <div>Dragging: {draggingConnection() ? 'Yes' : 'No'}</div>
            <div>Connections: {connections().length}</div>
            <div>Zoom: {Math.round(zoom() * 100)}%</div>
          </div>

          {/* Connection Lines SVG */}
          <svg 
            class="absolute inset-0 pointer-events-none w-full h-full"
            style={{
              overflow: 'visible'
            }}
          >
            {/* Existing Connections */}
            <For each={connections()}>
              {(connection) => {
                return (
                  <ConnectionLine
                    connection={connection}
                    nodes={nodes()}
                    getSocketScreenPosition={getSocketScreenPosition}
                    draggedNodeId={draggedNode()?.id}
                    draggedNodeTransform={draggedNodeTransform}
                    zoom={zoom}
                    pan={pan}
                  />
                );
              }}
            </For>
            
            {/* Dragging Connection */}
            <Show when={draggingConnection()}>
              {() => {
                const fromPos = getSocketScreenPosition(draggingConnection().nodeId, draggingConnection().socketId, 'output');
                if (!fromPos) return null;
                
                const fromX = fromPos.x;
                const fromY = fromPos.y;
                const toX = dragConnectionEnd().x;
                const toY = dragConnectionEnd().y;
                
                const controlOffset = Math.max(80, Math.abs(toX - fromX) * 0.4);
                const pathData = `M ${fromX} ${fromY} C ${fromX + controlOffset} ${fromY} ${toX - controlOffset} ${toY} ${toX} ${toY}`;
                
                return (
                  <path
                    d={pathData}
                    stroke="#60a5fa"
                    stroke-width="3"
                    fill="none"
                    stroke-dasharray="5,5"
                    class="opacity-80"
                  />
                );
              }}
            </Show>
          </svg>
        </div>
      </div>
    </div>
  );
}