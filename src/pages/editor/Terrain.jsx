import { createSignal, For } from 'solid-js';
import { 
  Plus, Settings, Upload, Download, ArrowUp, ArrowDown,
  Grid, Maximize, Edit, Copy, Trash 
} from '@/ui/icons';

function Terrain() {
  const [selectedTool, setSelectedTool] = createSignal('sculpt');
  const [brushSize, setBrushSize] = createSignal(10);
  const [brushStrength, setBrushStrength] = createSignal(0.5);
  const [terrainSize, setTerrainSize] = createSignal(512);
  
  const [layers, setLayers] = createSignal([
    {
      id: 1,
      name: 'Grass',
      texture: 'grass_diffuse.jpg',
      visible: true,
      opacity: 1.0,
      selected: true
    },
    {
      id: 2,
      name: 'Rock',
      texture: 'rock_diffuse.jpg',
      visible: true,
      opacity: 0.8,
      selected: false
    },
    {
      id: 3,
      name: 'Sand',
      texture: 'sand_diffuse.jpg',
      visible: false,
      opacity: 0.6,
      selected: false
    }
  ]);

  const tools = [
    { id: 'sculpt', name: 'Sculpt', icon: '🎨', description: 'Raise and lower terrain' },
    { id: 'smooth', name: 'Smooth', icon: '🌊', description: 'Smooth terrain surface' },
    { id: 'paint', name: 'Paint', icon: '🖌️', description: 'Paint terrain textures' },
    { id: 'noise', name: 'Noise', icon: '⚡', description: 'Add procedural noise' },
    { id: 'flatten', name: 'Flatten', icon: '📏', description: 'Flatten terrain areas' },
    { id: 'erode', name: 'Erode', icon: '💧', description: 'Simulate erosion effects' }
  ];

  const selectLayer = (layer) => {
    setLayers(prev => prev.map(l => ({
      ...l,
      selected: l.id === layer.id
    })));
  };

  const toggleLayerVisibility = (layerId) => {
    setLayers(prev => prev.map(l => 
      l.id === layerId ? { ...l, visible: !l.visible } : l
    ));
  };

  const selectedLayer = () => layers().find(l => l.selected);

  return (
    <div class="h-full flex flex-col bg-base-100">
      {/* Terrain Header */}
      <div class="flex items-center justify-between p-3 border-b border-base-300">
        <div class="flex items-center space-x-2">
          <div class="w-4 h-4 bg-gradient-to-r from-green-600 to-yellow-500 rounded-full"></div>
          <span class="text-sm font-medium text-base-content">Terrain Editor</span>
        </div>
        
        <div class="flex items-center space-x-1">
          <button class="btn btn-xs btn-primary" title="Generate Terrain">
            <Plus class="w-3 h-3" />
          </button>
          <button class="btn btn-xs btn-ghost" title="Import Heightmap">
            <Upload class="w-3 h-3" />
          </button>
          <button class="btn btn-xs btn-ghost" title="Export Heightmap">
            <Download class="w-3 h-3" />
          </button>
          <button class="btn btn-xs btn-ghost" title="Settings">
            <Settings class="w-3 h-3" />
          </button>
        </div>
      </div>

      <div class="flex-1 flex">
        {/* Tools Panel */}
        <div class="w-48 border-r border-base-300 flex flex-col">
          {/* Sculpting Tools */}
          <div class="p-2 border-b border-base-300">
            <div class="text-xs text-base-content/60 uppercase tracking-wide">Tools</div>
          </div>
          
          <div class="flex-1 overflow-y-auto space-y-1 p-2">
            <For each={tools}>
              {(tool) => (
                <button
                  class={`w-full p-2 rounded text-left hover:bg-base-200 transition-colors ${
                    selectedTool() === tool.id ? 'bg-primary text-primary-content' : ''
                  }`}
                  onClick={() => setSelectedTool(tool.id)}
                >
                  <div class="flex items-center space-x-2">
                    <span class="text-sm">{tool.icon}</span>
                    <span class="text-xs font-medium">{tool.name}</span>
                  </div>
                  <div class="text-[10px] text-base-content/40 mt-1">
                    {tool.description}
                  </div>
                </button>
              )}
            </For>
          </div>

          {/* Brush Settings */}
          <div class="border-t border-base-300 p-3 space-y-3">
            <div class="text-xs text-base-content/60 uppercase tracking-wide">Brush</div>
            
            <div class="space-y-2">
              <div class="flex items-center justify-between">
                <label class="text-xs text-base-content/60">Size</label>
                <input
                  type="number"
                  class="input input-xs input-bordered w-16 text-xs"
                  value={brushSize()}
                  onChange={(e) => setBrushSize(parseInt(e.target.value) || 1)}
                />
              </div>
              <input
                type="range"
                min="1"
                max="50"
                value={brushSize()}
                onChange={(e) => setBrushSize(parseInt(e.target.value))}
                class="range range-xs range-primary"
              />
            </div>

            <div class="space-y-2">
              <div class="flex items-center justify-between">
                <label class="text-xs text-base-content/60">Strength</label>
                <input
                  type="number"
                  step="0.1"
                  min="0"
                  max="2"
                  class="input input-xs input-bordered w-16 text-xs"
                  value={brushStrength()}
                  onChange={(e) => setBrushStrength(parseFloat(e.target.value) || 0)}
                />
              </div>
              <input
                type="range"
                min="0"
                max="2"
                step="0.1"
                value={brushStrength()}
                onChange={(e) => setBrushStrength(parseFloat(e.target.value))}
                class="range range-xs range-primary"
              />
            </div>
          </div>
        </div>

        {/* Layers Panel */}
        <div class="w-64 border-r border-base-300 flex flex-col">
          <div class="p-2 border-b border-base-300 flex items-center justify-between">
            <div class="text-xs text-base-content/60 uppercase tracking-wide">Layers</div>
            <button class="btn btn-xs btn-ghost" title="Add Layer">
              <Plus class="w-3 h-3" />
            </button>
          </div>
          
          <div class="flex-1 overflow-y-auto">
            <For each={layers()}>
              {(layer) => (
                <div
                  class={`p-2 cursor-pointer hover:bg-base-200 border-l-2 ${
                    layer.selected ? 'bg-base-200 border-primary' : 'border-transparent'
                  }`}
                  onClick={() => selectLayer(layer)}
                >
                  <div class="flex items-center space-x-2">
                    <button
                      class={`w-4 h-4 rounded border-2 flex items-center justify-center ${
                        layer.visible 
                          ? 'border-primary bg-primary text-primary-content' 
                          : 'border-base-300'
                      }`}
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleLayerVisibility(layer.id);
                      }}
                    >
                      {layer.visible && <span class="text-[8px]">✓</span>}
                    </button>
                    
                    <div class="flex-1 min-w-0">
                      <div class="flex items-center justify-between">
                        <span class="text-xs font-medium truncate">{layer.name}</span>
                        <div class="flex space-x-1">
                          <button class="btn btn-xs btn-ghost p-0 w-4 h-4">
                            <Edit class="w-2 h-2" />
                          </button>
                          <button class="btn btn-xs btn-ghost p-0 w-4 h-4">
                            <Copy class="w-2 h-2" />
                          </button>
                          <button class="btn btn-xs btn-ghost p-0 w-4 h-4 text-error">
                            <Trash class="w-2 h-2" />
                          </button>
                        </div>
                      </div>
                      
                      <div class="text-[10px] text-base-content/40 truncate mt-1">
                        {layer.texture}
                      </div>
                      
                      <div class="flex items-center space-x-2 mt-2">
                        <span class="text-[10px] text-base-content/40">Opacity:</span>
                        <input
                          type="range"
                          min="0"
                          max="1"
                          step="0.1"
                          value={layer.opacity}
                          class="range range-xs flex-1"
                          onClick={(e) => e.stopPropagation()}
                          onChange={(e) => {
                            e.stopPropagation();
                            const newOpacity = parseFloat(e.target.value);
                            setLayers(prev => prev.map(l => 
                              l.id === layer.id ? { ...l, opacity: newOpacity } : l
                            ));
                          }}
                        />
                        <span class="text-[10px] text-base-content/60 w-8">
                          {Math.round(layer.opacity * 100)}%
                        </span>
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </For>
          </div>
        </div>

        {/* Properties Panel */}
        <div class="flex-1 flex flex-col">
          {/* Terrain Info */}
          <div class="p-3 border-b border-base-300">
            <h3 class="text-sm font-medium">Terrain Properties</h3>
          </div>

          <div class="flex-1 overflow-y-auto p-3 space-y-4">
            {/* Terrain Settings */}
            <div class="space-y-2">
              <h4 class="text-xs font-medium text-base-content/80">Terrain</h4>
              <div class="space-y-2">
                <div class="flex items-center justify-between">
                  <label class="text-xs text-base-content/60">Size</label>
                  <select
                    class="select select-xs select-bordered text-xs"
                    value={terrainSize()}
                    onChange={(e) => setTerrainSize(parseInt(e.target.value))}
                  >
                    <option value={256}>256x256</option>
                    <option value={512}>512x512</option>
                    <option value={1024}>1024x1024</option>
                    <option value={2048}>2048x2048</option>
                  </select>
                </div>
                <div class="flex items-center justify-between">
                  <label class="text-xs text-base-content/60">Height Scale</label>
                  <input
                    type="number"
                    step="0.1"
                    class="input input-xs input-bordered w-16 text-xs"
                    value={50}
                  />
                </div>
                <div class="flex items-center justify-between">
                  <label class="text-xs text-base-content/60">World Size</label>
                  <input
                    type="number"
                    class="input input-xs input-bordered w-16 text-xs"
                    value={1000}
                  />
                </div>
              </div>
            </div>

            {/* Generation Settings */}
            <div class="space-y-2">
              <h4 class="text-xs font-medium text-base-content/80">Generation</h4>
              <div class="space-y-2">
                <button class="btn btn-xs btn-primary w-full">Generate from Noise</button>
                <button class="btn btn-xs btn-ghost w-full">Generate from Image</button>
                <button class="btn btn-xs btn-ghost w-full">Flatten Terrain</button>
                <button class="btn btn-xs btn-error w-full">Clear Terrain</button>
              </div>
            </div>

            {/* Layer Properties */}
            {selectedLayer() && (
              <div class="space-y-2">
                <h4 class="text-xs font-medium text-base-content/80">
                  Layer: {selectedLayer().name}
                </h4>
                <div class="space-y-2">
                  <div class="flex items-center justify-between">
                    <label class="text-xs text-base-content/60">Tiling</label>
                    <input
                      type="number"
                      step="0.1"
                      class="input input-xs input-bordered w-16 text-xs"
                      value={1.0}
                    />
                  </div>
                  <div class="flex items-center justify-between">
                    <label class="text-xs text-base-content/60">Metallic</label>
                    <input
                      type="number"
                      step="0.1"
                      min="0"
                      max="1"
                      class="input input-xs input-bordered w-16 text-xs"
                      value={0.0}
                    />
                  </div>
                  <div class="flex items-center justify-between">
                    <label class="text-xs text-base-content/60">Roughness</label>
                    <input
                      type="number"
                      step="0.1"
                      min="0"
                      max="1"
                      class="input input-xs input-bordered w-16 text-xs"
                      value={0.8}
                    />
                  </div>
                </div>
              </div>
            )}

            {/* Statistics */}
            <div class="space-y-2">
              <h4 class="text-xs font-medium text-base-content/80">Statistics</h4>
              <div class="space-y-1 text-xs text-base-content/60">
                <div class="flex justify-between">
                  <span>Vertices:</span>
                  <span>{(terrainSize() * terrainSize()).toLocaleString()}</span>
                </div>
                <div class="flex justify-between">
                  <span>Triangles:</span>
                  <span>{((terrainSize() - 1) * (terrainSize() - 1) * 2).toLocaleString()}</span>
                </div>
                <div class="flex justify-between">
                  <span>Memory:</span>
                  <span>{Math.round(terrainSize() * terrainSize() * 4 / 1024 / 1024)}MB</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default Terrain;