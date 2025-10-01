import { createSignal, Show, onMount, onCleanup } from 'solid-js';
import { IconMountain, IconBrush, IconCircle, IconSquare, IconPlayerPlay, IconPlayerStop, IconSun, IconBulb } from '@tabler/icons-solidjs';
import { renderActions } from '@/render/store.jsx';
import { editorStore, editorActions } from '@/layout/stores/EditorStore.jsx';
import { currentTool } from './index.jsx';

const TerrainPropertiesPanel = (props) => {
  const [terrainData, setTerrainData] = createSignal(null);
  const [brushSize, setBrushSize] = createSignal(8);
  const [brushStrength, setBrushStrength] = createSignal(0.2);
  const [selectedTool, setSelectedTool] = createSignal('raise');

  onMount(() => {
    if (props.selectedObject && props.selectedObject._terrainData) {
      const data = props.selectedObject._terrainData;
      setTerrainData(data);
      setBrushSize(data.brushSize || 8);
      setBrushStrength(data.brushStrength || 0.2);
    }
  });

  const updateTerrainData = (updates) => {
    if (props.selectedObject && props.selectedObject._terrainData) {
      Object.assign(props.selectedObject._terrainData, updates);
      setTerrainData({ ...props.selectedObject._terrainData });
    }
  };

  const handleBrushSizeChange = (e) => {
    const value = parseFloat(e.target.value);
    setBrushSize(value);
    updateTerrainData({ brushSize: value });
  };

  const handleBrushStrengthChange = (e) => {
    const value = parseFloat(e.target.value);
    setBrushStrength(value);
    updateTerrainData({ brushStrength: value });
  };

  // Check if we're in sculpting mode
  const isTerrainEditMode = () => editorStore.ui.currentMode === 'sculpting';

  const handleToolSelect = (tool) => {
    setSelectedTool(tool);
    
    // Switch to the new tool
    switchTerrainTool(tool);
  };
  
  const switchTerrainTool = (tool) => {
    const event = new CustomEvent('engine:switch-terrain-tool', { 
      detail: { tool } 
    });
    document.dispatchEvent(event);
  };

  const resetTerrain = () => {
    if (props.selectedObject && props.selectedObject._terrainData) {
      // Generate new heightmap
      const data = props.selectedObject._terrainData;
      const newHeightmap = generateFlatHeightmap(data.subdivisions + 1, data.subdivisions + 1);
      
      // Update terrain mesh
      updateTerrainGeometry(props.selectedObject, newHeightmap);
      updateTerrainData({ heightmapData: newHeightmap });
      
      editorActions.addConsoleMessage('Terrain reset to flat', 'info');
    }
  };

  const generateFlatHeightmap = (width, height) => {
    return new Array(width * height).fill(0);
  };

  const updateTerrainGeometry = (terrainMesh, heightmapData) => {
    // This would update the mesh vertices based on new heightmap data
    // For now, we'll just log that it would update
    console.log('Would update terrain geometry with new heightmap data');
  };

  return (
    <div class="h-full overflow-y-auto p-4 space-y-4">
      <Show when={terrainData()}>
        {/* Terrain Info */}
        <div class="bg-base-100 rounded-lg p-4">
          <div class="flex items-center gap-2 mb-3">
            <IconMountain class="w-4 h-4 text-primary" />
            <h3 class="text-sm font-medium">Terrain Properties</h3>
          </div>
          
          <div class="space-y-3 text-xs">
            <div class="flex justify-between">
              <span class="text-base-content/60">Size:</span>
              <span>{terrainData().size}m</span>
            </div>
            <div class="flex justify-between">
              <span class="text-base-content/60">Subdivisions:</span>
              <span>{terrainData().subdivisions}</span>
            </div>
            <div class="flex justify-between">
              <span class="text-base-content/60">Vertices:</span>
              <span>{Math.pow(terrainData().subdivisions + 1, 2)}</span>
            </div>
          </div>
        </div>

        {/* Sculpting Mode Info */}
        <div class="bg-base-100 rounded-lg p-4">
          <div class="flex items-center justify-between mb-3">
            <div class="flex items-center gap-2">
              <IconBrush class="w-4 h-4 text-primary" />
              <h3 class="text-sm font-medium">Sculpting Mode</h3>
            </div>
            <div class={`badge badge-sm ${isTerrainEditMode() ? 'badge-success' : 'badge-outline'}`}>
              {isTerrainEditMode() ? 'Active' : 'Inactive'}
            </div>
          </div>
          
          {!isTerrainEditMode() && (
            <div class="text-xs text-base-content/60 p-3 bg-base-200 rounded">
              Switch to <strong>Sculpting</strong> mode from the toolbar dropdown to start terrain editing
            </div>
          )}
        </div>

        {/* Sculpting Tools */}
        <div class="bg-base-100 rounded-lg p-4">
          <div class="flex items-center gap-2 mb-3">
            <IconBrush class="w-4 h-4 text-primary" />
            <h3 class="text-sm font-medium">Sculpting Tools</h3>
          </div>
          
          <div class="grid grid-cols-3 gap-2 mb-4">
            <button
              onClick={() => handleToolSelect('raise')}
              class={`p-3 rounded-lg border transition-all ${
                (selectedTool() === 'raise' || currentTool() === 'raise') 
                  ? 'bg-primary text-primary-content border-primary' 
                  : 'bg-base-200 border-base-300 hover:bg-base-300'
              }`}
            >
              <div class="flex flex-col items-center gap-1">
                <IconMountain class="w-4 h-4" />
                <span class="text-xs">Raise</span>
              </div>
            </button>
            
            <button
              onClick={() => handleToolSelect('lower')}
              class={`p-3 rounded-lg border transition-all ${
                (selectedTool() === 'lower' || currentTool() === 'lower') 
                  ? 'bg-primary text-primary-content border-primary' 
                  : 'bg-base-200 border-base-300 hover:bg-base-300'
              }`}
            >
              <div class="flex flex-col items-center gap-1">
                <IconSquare class="w-4 h-4" />
                <span class="text-xs">Lower</span>
              </div>
            </button>
            
            <button
              onClick={() => handleToolSelect('smooth')}
              class={`p-3 rounded-lg border transition-all ${
                (selectedTool() === 'smooth' || currentTool() === 'smooth') 
                  ? 'bg-primary text-primary-content border-primary' 
                  : 'bg-base-200 border-base-300 hover:bg-base-300'
              }`}
            >
              <div class="flex flex-col items-center gap-1">
                <IconCircle class="w-4 h-4" />
                <span class="text-xs">Smooth</span>
              </div>
            </button>
            
            <button
              onClick={() => handleToolSelect('flatten')}
              class={`p-3 rounded-lg border transition-all ${
                (selectedTool() === 'flatten' || currentTool() === 'flatten') 
                  ? 'bg-primary text-primary-content border-primary' 
                  : 'bg-base-200 border-base-300 hover:bg-base-300'
              }`}
            >
              <div class="flex flex-col items-center gap-1">
                <IconBrush class="w-4 h-4" />
                <span class="text-xs">Flatten</span>
              </div>
            </button>
            
            <button
              onClick={() => handleToolSelect('paint')}
              class={`p-3 rounded-lg border transition-all ${
                (selectedTool() === 'paint' || currentTool() === 'paint') 
                  ? 'bg-primary text-primary-content border-primary' 
                  : 'bg-base-200 border-base-300 hover:bg-base-300'
              }`}
            >
              <div class="flex flex-col items-center gap-1">
                <IconSun class="w-4 h-4" />
                <span class="text-xs">Paint</span>
              </div>
            </button>
            
            <button
              onClick={() => handleToolSelect('noise')}
              class={`p-3 rounded-lg border transition-all ${
                (selectedTool() === 'noise' || currentTool() === 'noise') 
                  ? 'bg-primary text-primary-content border-primary' 
                  : 'bg-base-200 border-base-300 hover:bg-base-300'
              }`}
            >
              <div class="flex flex-col items-center gap-1">
                <IconBulb class="w-4 h-4" />
                <span class="text-xs">Noise</span>
              </div>
            </button>
          </div>
        </div>

        {/* Brush Settings */}
        <div class="bg-base-100 rounded-lg p-4">
          <div class="flex items-center justify-between mb-3">
            <h3 class="text-sm font-medium">Brush Settings</h3>
            <div class="text-xs text-base-content/60">Ctrl+Scroll to resize</div>
          </div>
          
          <div class="space-y-4">
            <div>
              <div class="flex justify-between items-center mb-2">
                <label class="text-xs text-base-content/60">Size</label>
                <span class="text-xs">{brushSize()}</span>
              </div>
              <input
                type="range"
                min="1"
                max="32"
                step="0.5"
                value={brushSize()}
                onInput={handleBrushSizeChange}
                class="range range-primary range-xs w-full"
              />
            </div>
            
            <div>
              <div class="flex justify-between items-center mb-2">
                <label class="text-xs text-base-content/60">Strength</label>
                <span class="text-xs">{(brushStrength() * 100).toFixed(0)}%</span>
              </div>
              <input
                type="range"
                min="0.01"
                max="1.0"
                step="0.01"
                value={brushStrength()}
                onInput={handleBrushStrengthChange}
                class="range range-primary range-xs w-full"
              />
            </div>
          </div>
        </div>

        {/* Terrain Actions */}
        <div class="bg-base-100 rounded-lg p-4">
          <h3 class="text-sm font-medium mb-3">Terrain Actions</h3>
          
          <div class="space-y-2">
            <button
              onClick={resetTerrain}
              class="w-full btn btn-sm btn-outline"
            >
              Reset to Flat
            </button>
            
            <button
              onClick={() => editorActions.addConsoleMessage('Import heightmap not yet implemented', 'info')}
              class="w-full btn btn-sm btn-outline"
            >
              Import Heightmap
            </button>
            
            <button
              onClick={() => editorActions.addConsoleMessage('Export heightmap not yet implemented', 'info')}
              class="w-full btn btn-sm btn-outline"
            >
              Export Heightmap
            </button>
          </div>
        </div>
      </Show>
      
      <Show when={!terrainData()}>
        <div class="h-full flex flex-col items-center justify-center text-center text-base-content/60 p-4">
          <IconMountain class="w-8 h-8 mb-2 opacity-40" />
          <p class="text-xs">Select a terrain object to edit its properties</p>
        </div>
      </Show>
    </div>
  );
};

export default TerrainPropertiesPanel;