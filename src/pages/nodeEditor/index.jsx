import { onMount, onCleanup, createSignal, createMemo } from 'solid-js';
import { usePluginAPI } from '@/api/plugin';
import { IconBrandGit } from '@tabler/icons-solidjs';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { NodeLibrary } from './NodeLibrary';
import NodeCanvas from './NodeCanvas';
import NodeRenderer from './NodeRenderer';
import useConnectionHandler from './ConnectionHandler';
import NodeLibraryPanel from './NodeLibraryPanel';
import useDragDropHandler from './DragDropHandler';

const getBabylonScene = () => window._cleanBabylonScene;
const NodeIcon = () => <div>🔗</div>;

const NodeEditor = (props) => {
  const { nodeEditor } = editorStore;
  const { 
    getNodeGraph, setNodeGraph, updateNodeGraph, addConnectionAndGenerateProperties, removeConnectionFromGraph,
    addPropertySection, bindNodeToProperty, initializeObjectProperties 
  } = editorActions;
  
  let containerRef;

  const getObjectName = () => {
    const scene = getBabylonScene();
    if (!scene) return props.objectId;

    const allObjects = [
      ...(scene.meshes || []),
      ...(scene.transformNodes || []),
      ...(scene.lights || []),
      ...(scene.cameras || [])
    ];
    
    const babylonObject = allObjects.find(obj => 
      (obj.uniqueId || obj.name) === props.objectId
    );
    
    return babylonObject ? (babylonObject.name || props.objectId) : props.objectId;
  };

  const initializeGraph = () => {
    const existingGraph = getNodeGraph(props.objectId);
    if (existingGraph) {
      return existingGraph;
    }

    const defaultGraph = {
      nodes: [],
      connections: [],
      viewTransform: { x: 0, y: 0, scale: 1 }
    };

    setNodeGraph(props.objectId, defaultGraph);
    initializeObjectProperties(props.objectId);
    
    return defaultGraph;
  };

  const currentGraph = createMemo(() => {
    return nodeEditor.graphs[props.objectId] || initializeGraph();
  });

  const nodes = createMemo(() => currentGraph().nodes || []);
  const connections = createMemo(() => currentGraph().connections || []);
  const viewTransform = createMemo(() => currentGraph().viewTransform || { x: 0, y: 0, scale: 1 });
  
  const [selectedNodes, setSelectedNodes] = createSignal(new Set());
  const [tempConnection, setTempConnection] = createSignal(null);
  const [contextMenu, setContextMenu] = createSignal(null);
  const [activeSubmenu, setActiveSubmenu] = createSignal(null);
  const [submenuPosition, setSubmenuPosition] = createSignal({ top: 0 });
  const [viewportSize, setViewportSize] = createSignal({ width: 800, height: 600 });

  onMount(() => {
    const updateSize = () => {
      if (containerRef) {
        const rect = containerRef.getBoundingClientRect();
        setViewportSize({ width: rect.width, height: rect.height });
      }
    };

    updateSize();
    window.addEventListener('resize', updateSize);
    
    onCleanup(() => {
      window.removeEventListener('resize', updateSize);
    });
  });

  const addNode = (nodeType, position) => {
    console.log('Adding node:', nodeType, 'at position:', position);
    
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

    console.log('Created new node:', newNode);
    const updatedNodes = [...nodes(), newNode];
    console.log('Updated nodes array:', updatedNodes);
    updateNodeGraph(props.objectId, { nodes: updatedNodes });
    initializeObjectProperties(props.objectId);
  };

  const handleContextMenu = (e) => {
    e.preventDefault();
    const worldPos = dragDropHandler.screenToWorld(e.clientX, e.clientY);
    const rect = containerRef?.getBoundingClientRect();
    if (!rect) return;
    
    const menuData = {
      position: { 
        x: e.clientX - rect.left, 
        y: e.clientY - rect.top 
      },
      worldPosition: worldPos
    };
    
    console.log('Setting context menu:', menuData);
    setContextMenu(menuData);
  };

  const dragDropHandler = useDragDropHandler({
    containerRef,
    viewTransform,
    nodes,
    selectedNodes,
    setSelectedNodes,
    setTempConnection,
    updateNodeGraph,
    addConnectionAndGenerateProperties,
    objectId: props.objectId,
    contextMenu,
    setContextMenu,
    setActiveSubmenu
  });

  const connectionHandler = useConnectionHandler({
    connections,
    nodes,
    removeConnectionFromGraph,
    objectId: props.objectId
  });

  const renderNode = (node) => (
    <NodeRenderer 
      node={node} 
      selectedNodes={selectedNodes}
      objectId={props.objectId}
    />
  );

  return (
    <>
      <NodeCanvas
        viewportSize={viewportSize}
        viewTransform={viewTransform}
        nodes={nodes}
        connections={connections}
        selectedNodes={selectedNodes}
        tempConnection={tempConnection}
        contextMenu={contextMenu}
        onMouseDown={dragDropHandler.handleMouseDown}
        onMouseMove={dragDropHandler.handleMouseMove}
        onMouseUp={dragDropHandler.handleMouseUp}
        onContextMenu={handleContextMenu}
        onWheel={dragDropHandler.handleWheel}
        setContextMenu={setContextMenu}
        setActiveSubmenu={setActiveSubmenu}
        setContainerRef={(el) => { containerRef = el; }}
        renderNode={renderNode}
        renderConnections={connectionHandler.renderConnections}
        getConnectionPath={connectionHandler.getConnectionPath}
        updateNodeGraph={updateNodeGraph}
        objectId={props.objectId}
      />
      
      <NodeLibraryPanel
        contextMenu={contextMenu}
        activeSubmenu={activeSubmenu}
        submenuPosition={submenuPosition}
        setActiveSubmenu={setActiveSubmenu}
        setSubmenuPosition={setSubmenuPosition}
        setContextMenu={setContextMenu}
        addNode={addNode}
      />
    </>
  );
};

export { NodeEditor };

export default function NodeEditorPage() {
  onMount(() => {
    console.log('[NodeEditorPage] Initializing node editor page...');
    const api = usePluginAPI();
    
    api.viewport('node-editor', {
      label: 'Node Editor',
      component: NodeEditor,
      icon: NodeIcon,
      description: 'Visual node-based scripting interface'
    });
    
    api.button('node-editor-create', {
      title: 'Node Editor',
      icon: IconBrandGit,
      onClick: () => {
        console.log('[NodeEditorPage] Switch to Node Editor page');
      },
      section: 'main',
      order: 15
    });

    console.log('[NodeEditorPage] Node editor page initialized');
    
    onCleanup(() => {
      console.log('[NodeEditorPage] Cleaning up node editor page...');
    });
  });

  return null;
}