import { createSignal, For } from 'solid-js';
import { 
  Plus, Settings, Search, Grid, Maximize, Minimize, 
  Copy, Trash, ArrowUp, ArrowDown, Link, Unlink 
} from '@/ui/icons';

function Nodes() {
  const [searchTerm, setSearchTerm] = createSignal('');
  const [selectedCategory, setSelectedCategory] = createSignal('all');
  const [nodes, setNodes] = createSignal([
    {
      id: 1,
      name: 'Material Output',
      type: 'output',
      category: 'output',
      x: 400,
      y: 200,
      inputs: ['Surface', 'Volume', 'Displacement'],
      outputs: [],
      selected: false
    },
    {
      id: 2,
      name: 'Principled BSDF',
      type: 'shader',
      category: 'shader',
      x: 200,
      y: 150,
      inputs: ['Base Color', 'Metallic', 'Roughness', 'Normal'],
      outputs: ['BSDF'],
      selected: true
    },
    {
      id: 3,
      name: 'Image Texture',
      type: 'texture',
      category: 'input',
      x: 50,
      y: 100,
      inputs: ['Vector'],
      outputs: ['Color', 'Alpha'],
      selected: false
    }
  ]);

  const [connections, setConnections] = createSignal([
    { id: 1, from: { nodeId: 3, output: 'Color' }, to: { nodeId: 2, input: 'Base Color' } },
    { id: 2, from: { nodeId: 2, output: 'BSDF' }, to: { nodeId: 1, input: 'Surface' } }
  ]);

  const categories = [
    { id: 'all', name: 'All', icon: '📦' },
    { id: 'input', name: 'Input', icon: '📥' },
    { id: 'output', name: 'Output', icon: '📤' },
    { id: 'shader', name: 'Shader', icon: '🎨' },
    { id: 'texture', name: 'Texture', icon: '🖼️' },
    { id: 'math', name: 'Math', icon: '🧮' },
    { id: 'converter', name: 'Converter', icon: '🔄' },
    { id: 'vector', name: 'Vector', icon: '📐' },
    { id: 'color', name: 'Color', icon: '🌈' }
  ];

  const nodeLibrary = [
    { name: 'Mix RGB', type: 'color', category: 'color' },
    { name: 'ColorRamp', type: 'converter', category: 'converter' },
    { name: 'Noise Texture', type: 'texture', category: 'input' },
    { name: 'Voronoi Texture', type: 'texture', category: 'input' },
    { name: 'Math', type: 'math', category: 'math' },
    { name: 'Vector Math', type: 'vector', category: 'vector' },
    { name: 'Mapping', type: 'vector', category: 'vector' },
    { name: 'Fresnel', type: 'input', category: 'input' },
    { name: 'Layer Weight', type: 'input', category: 'input' },
    { name: 'Ambient Occlusion', type: 'input', category: 'input' }
  ];

  const filteredNodes = () => {
    return nodeLibrary.filter(node => {
      const matchesSearch = !searchTerm() || 
        node.name.toLowerCase().includes(searchTerm().toLowerCase());
      const matchesCategory = selectedCategory() === 'all' || 
        node.category === selectedCategory();
      return matchesSearch && matchesCategory;
    });
  };

  const selectedNode = () => nodes().find(n => n.selected);

  const selectNode = (nodeId) => {
    setNodes(prev => prev.map(n => ({
      ...n,
      selected: n.id === nodeId
    })));
  };

  const deleteSelectedNode = () => {
    const selected = selectedNode();
    if (selected) {
      setNodes(prev => prev.filter(n => n.id !== selected.id));
      setConnections(prev => prev.filter(c => 
        c.from.nodeId !== selected.id && c.to.nodeId !== selected.id
      ));
    }
  };

  const duplicateSelectedNode = () => {
    const selected = selectedNode();
    if (selected) {
      const newNode = {
        ...selected,
        id: Date.now(),
        name: `${selected.name} Copy`,
        x: selected.x + 50,
        y: selected.y + 50,
        selected: false
      };
      setNodes(prev => [...prev, newNode]);
    }
  };

  const getNodeColor = (category) => {
    switch (category) {
      case 'input': return 'bg-blue-500';
      case 'output': return 'bg-green-500';
      case 'shader': return 'bg-purple-500';
      case 'texture': return 'bg-orange-500';
      case 'math': return 'bg-red-500';
      case 'converter': return 'bg-yellow-500';
      case 'vector': return 'bg-pink-500';
      case 'color': return 'bg-cyan-500';
      default: return 'bg-base-300';
    }
  };

  return (
    <div class="h-full flex flex-col bg-base-100">
      {/* Nodes Header */}
      <div class="flex items-center justify-between p-3 border-b border-base-300">
        <div class="flex items-center space-x-2">
          <div class="w-4 h-4 bg-gradient-to-r from-purple-400 to-pink-500 rounded-full"></div>
          <span class="text-sm font-medium text-base-content">Node Editor</span>
        </div>
        
        <div class="flex items-center space-x-1">
          <button class="btn btn-xs btn-primary" title="Add Node">
            <Plus class="w-3 h-3" />
          </button>
          <button class="btn btn-xs btn-ghost" title="Auto Layout">
            <Grid class="w-3 h-3" />
          </button>
          <button class="btn btn-xs btn-ghost" title="Fit View">
            <Maximize class="w-3 h-3" />
          </button>
          <button class="btn btn-xs btn-ghost" title="Settings">
            <Settings class="w-3 h-3" />
          </button>
        </div>
      </div>

      <div class="flex-1 flex">
        {/* Node Library */}
        <div class="w-56 border-r border-base-300 flex flex-col">
          {/* Search */}
          <div class="p-2 border-b border-base-300">
            <div class="flex items-center space-x-2">
              <Search class="w-3 h-3 text-base-content/40" />
              <input
                type="text"
                placeholder="Search nodes..."
                class="input input-xs input-ghost flex-1 text-xs"
                value={searchTerm()}
                onInput={(e) => setSearchTerm(e.target.value)}
              />
            </div>
          </div>

          {/* Categories */}
          <div class="border-b border-base-300">
            <div class="p-2">
              <div class="text-xs text-base-content/60 uppercase tracking-wide mb-2">Categories</div>
              <div class="space-y-1">
                <For each={categories}>
                  {(category) => (
                    <button
                      class={`w-full text-left p-1 px-2 rounded text-xs hover:bg-base-200 ${
                        selectedCategory() === category.id ? 'bg-primary text-primary-content' : ''
                      }`}
                      onClick={() => setSelectedCategory(category.id)}
                    >
                      <span class="mr-2">{category.icon}</span>
                      {category.name}
                    </button>
                  )}
                </For>
              </div>
            </div>
          </div>

          {/* Node List */}
          <div class="flex-1 overflow-y-auto p-2">
            <div class="text-xs text-base-content/60 uppercase tracking-wide mb-2">Nodes</div>
            <div class="space-y-1">
              <For each={filteredNodes()}>
                {(node) => (
                  <div
                    class="p-2 bg-base-200 rounded cursor-pointer hover:bg-base-300 transition-colors"
                    draggable="true"
                    onDragStart={(e) => {
                      e.dataTransfer.setData('application/json', JSON.stringify(node));
                    }}
                  >
                    <div class="flex items-center space-x-2">
                      <div class={`w-2 h-2 rounded-full ${getNodeColor(node.category)}`}></div>
                      <span class="text-xs font-medium">{node.name}</span>
                    </div>
                    <div class="text-[10px] text-base-content/40 mt-1 capitalize">
                      {node.category}
                    </div>
                  </div>
                )}
              </For>
            </div>
          </div>
        </div>

        {/* Node Graph */}
        <div class="flex-1 flex flex-col">
          {/* Graph Controls */}
          <div class="flex items-center justify-between p-2 border-b border-base-300 bg-base-200/50">
            <div class="flex items-center space-x-2">
              <span class="text-xs text-base-content/60">Zoom: 100%</span>
              <span class="text-xs text-base-content/60">|</span>
              <span class="text-xs text-base-content/60">
                {nodes().length} nodes, {connections().length} connections
              </span>
            </div>
            
            {selectedNode() && (
              <div class="flex items-center space-x-1">
                <span class="text-xs text-base-content/60">{selectedNode().name}</span>
                <button
                  class="btn btn-xs btn-ghost"
                  onClick={duplicateSelectedNode}
                  title="Duplicate"
                >
                  <Copy class="w-3 h-3" />
                </button>
                <button
                  class="btn btn-xs btn-ghost text-error"
                  onClick={deleteSelectedNode}
                  title="Delete"
                >
                  <Trash class="w-3 h-3" />
                </button>
              </div>
            )}
          </div>

          {/* Graph Canvas */}
          <div 
            class="flex-1 relative bg-base-100 overflow-hidden"
            style={{
              'background-image': 'radial-gradient(circle, oklch(var(--bc) / 0.1) 1px, transparent 1px)',
              'background-size': '20px 20px'
            }}
            onDragOver={(e) => e.preventDefault()}
            onDrop={(e) => {
              e.preventDefault();
              try {
                const nodeData = JSON.parse(e.dataTransfer.getData('application/json'));
                const rect = e.currentTarget.getBoundingClientRect();
                const x = e.clientX - rect.left;
                const y = e.clientY - rect.top;
                
                const newNode = {
                  id: Date.now(),
                  name: nodeData.name,
                  type: nodeData.type,
                  category: nodeData.category,
                  x: x - 50,
                  y: y - 25,
                  inputs: ['Input'],
                  outputs: ['Output'],
                  selected: false
                };
                
                setNodes(prev => [...prev, newNode]);
              } catch (error) {
                console.error('Error adding node:', error);
              }
            }}
          >
            {/* Render Connections */}
            <svg class="absolute inset-0 w-full h-full pointer-events-none" style={{ 'z-index': 1 }}>
              <For each={connections()}>
                {(connection) => {
                  const fromNode = nodes().find(n => n.id === connection.from.nodeId);
                  const toNode = nodes().find(n => n.id === connection.to.nodeId);
                  
                  if (!fromNode || !toNode) return null;
                  
                  const x1 = fromNode.x + 100; // Node width
                  const y1 = fromNode.y + 25;  // Node height / 2
                  const x2 = toNode.x;
                  const y2 = toNode.y + 25;
                  
                  const cp1x = x1 + (x2 - x1) * 0.5;
                  const cp2x = x2 - (x2 - x1) * 0.5;
                  
                  return (
                    <path
                      d={`M ${x1} ${y1} C ${cp1x} ${y1} ${cp2x} ${y2} ${x2} ${y2}`}
                      stroke="oklch(var(--p))"
                      stroke-width="2"
                      fill="none"
                      class="drop-shadow-sm"
                    />
                  );
                }}
              </For>
            </svg>

            {/* Render Nodes */}
            <For each={nodes()}>
              {(node) => (
                <div
                  class={`absolute bg-base-200 border rounded-lg shadow-lg cursor-pointer select-none ${
                    node.selected ? 'border-primary ring-2 ring-primary/20' : 'border-base-300'
                  }`}
                  style={{
                    left: `${node.x}px`,
                    top: `${node.y}px`,
                    width: '100px',
                    'z-index': node.selected ? 10 : 5
                  }}
                  onClick={() => selectNode(node.id)}
                >
                  {/* Node Header */}
                  <div class={`p-2 rounded-t-lg ${getNodeColor(node.category)} text-white`}>
                    <div class="text-xs font-medium truncate">{node.name}</div>
                  </div>
                  
                  {/* Node Body */}
                  <div class="p-2 space-y-1">
                    {/* Inputs */}
                    <For each={node.inputs}>
                      {(input, index) => (
                        <div class="flex items-center">
                          <div class="w-2 h-2 bg-base-400 rounded-full mr-1"></div>
                          <span class="text-[10px] text-base-content/60 truncate">{input}</span>
                        </div>
                      )}
                    </For>
                    
                    {/* Outputs */}
                    <For each={node.outputs}>
                      {(output, index) => (
                        <div class="flex items-center justify-end">
                          <span class="text-[10px] text-base-content/60 truncate">{output}</span>
                          <div class="w-2 h-2 bg-primary rounded-full ml-1"></div>
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              )}
            </For>

            {/* Empty State */}
            {nodes().length === 0 && (
              <div class="absolute inset-0 flex items-center justify-center">
                <div class="text-center text-base-content/40">
                  <div class="w-12 h-12 bg-gradient-to-r from-purple-400 to-pink-500 rounded-full mx-auto mb-4"></div>
                  <p class="text-sm mb-2">No nodes in graph</p>
                  <p class="text-xs">Drag nodes from the library to get started</p>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Properties Panel */}
        <div class="w-64 border-l border-base-300 flex flex-col">
          <div class="p-3 border-b border-base-300">
            <h3 class="text-sm font-medium">Properties</h3>
          </div>
          
          <div class="flex-1 overflow-y-auto p-3">
            {selectedNode() ? (
              <div class="space-y-4">
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Node</h4>
                  <div class="space-y-2">
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Name</label>
                      <input
                        type="text"
                        class="input input-xs input-bordered w-20 text-xs"
                        value={selectedNode().name}
                      />
                    </div>
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Type</label>
                      <span class="text-xs text-base-content/60 capitalize">{selectedNode().type}</span>
                    </div>
                  </div>
                </div>

                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Transform</h4>
                  <div class="space-y-2">
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">X</label>
                      <input
                        type="number"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={selectedNode().x}
                      />
                    </div>
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Y</label>
                      <input
                        type="number"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={selectedNode().y}
                      />
                    </div>
                  </div>
                </div>

                {/* Node-specific properties would go here */}
              </div>
            ) : (
              <div class="text-center text-base-content/40">
                <div class="w-8 h-8 bg-gradient-to-r from-purple-400 to-pink-500 rounded-full mx-auto mb-2"></div>
                <p class="text-xs">Select a node to edit properties</p>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export default Nodes;