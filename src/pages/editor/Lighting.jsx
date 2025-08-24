import { createSignal, For } from 'solid-js';
import { 
  Plus, Settings, Trash, Edit, Copy, Sun, Moon, 
  ArrowUp, ArrowDown, Eye, EyeOff, Maximize 
} from '@/ui/icons';

function Lighting() {
  const [lights, setLights] = createSignal([
    {
      id: 1,
      name: 'Key Light',
      type: 'directional',
      enabled: true,
      intensity: 5.0,
      color: '#ffffff',
      temperature: 5500,
      position: { x: 5, y: 10, z: 5 },
      rotation: { x: -45, y: 30, z: 0 },
      shadows: true,
      selected: true
    },
    {
      id: 2,
      name: 'Fill Light',
      type: 'spot',
      enabled: true,
      intensity: 2.5,
      color: '#ffeaa7',
      temperature: 3200,
      position: { x: -3, y: 5, z: 2 },
      rotation: { x: -30, y: -45, z: 0 },
      shadows: false,
      selected: false
    },
    {
      id: 3,
      name: 'Rim Light',
      type: 'spot',
      enabled: false,
      intensity: 8.0,
      color: '#74b9ff',
      temperature: 6500,
      position: { x: -5, y: 8, z: -3 },
      rotation: { x: -35, y: 145, z: 0 },
      shadows: true,
      selected: false
    },
    {
      id: 4,
      name: 'Ambient',
      type: 'area',
      enabled: true,
      intensity: 0.8,
      color: '#ddd6fe',
      temperature: 4000,
      position: { x: 0, y: 15, z: 0 },
      rotation: { x: -90, y: 0, z: 0 },
      shadows: false,
      selected: false
    }
  ]);

  const [environment, setEnvironment] = createSignal({
    hdri: 'studio_small_04.hdr',
    intensity: 1.2,
    rotation: 45,
    tint: '#ffffff',
    visible: true
  });

  const [globalSettings, setGlobalSettings] = createSignal({
    exposure: 1.0,
    gamma: 2.2,
    shadows: true,
    ambientOcclusion: true,
    volumetrics: false
  });

  const [selectedLight, setSelectedLight] = createSignal(lights()[0]);

  const lightTypes = [
    { id: 'directional', name: 'Directional', icon: '☀️' },
    { id: 'point', name: 'Point', icon: '💡' },
    { id: 'spot', name: 'Spot', icon: '🔦' },
    { id: 'area', name: 'Area', icon: '🔳' },
    { id: 'sun', name: 'Sun', icon: '🌞' },
    { id: 'sky', name: 'Sky', icon: '🌤️' }
  ];

  const selectLight = (light) => {
    setLights(prev => prev.map(l => ({
      ...l,
      selected: l.id === light.id
    })));
    setSelectedLight(light);
  };

  const toggleLight = (lightId) => {
    setLights(prev => prev.map(l => 
      l.id === lightId ? { ...l, enabled: !l.enabled } : l
    ));
  };

  const deleteLight = (lightId) => {
    setLights(prev => prev.filter(l => l.id !== lightId));
    if (selectedLight()?.id === lightId) {
      setSelectedLight(lights().length > 1 ? lights()[0] : null);
    }
  };

  const duplicateLight = (light) => {
    const newLight = {
      ...light,
      id: Date.now(),
      name: `${light.name} Copy`,
      position: {
        x: light.position.x + 1,
        y: light.position.y,
        z: light.position.z + 1
      },
      selected: false
    };
    setLights(prev => [...prev, newLight]);
  };

  const addLight = (type) => {
    const newLight = {
      id: Date.now(),
      name: `${type.charAt(0).toUpperCase() + type.slice(1)} Light`,
      type,
      enabled: true,
      intensity: 3.0,
      color: '#ffffff',
      temperature: 5500,
      position: { x: 0, y: 5, z: 0 },
      rotation: { x: 0, y: 0, z: 0 },
      shadows: true,
      selected: false
    };
    setLights(prev => [...prev, newLight]);
  };

  const getLightIcon = (type) => {
    const lightType = lightTypes.find(t => t.id === type);
    return lightType ? lightType.icon : '💡';
  };

  const getIntensityColor = (intensity) => {
    if (intensity > 5) return 'text-orange-500';
    if (intensity > 2) return 'text-yellow-500';
    return 'text-blue-500';
  };

  const kelvinToRGB = (kelvin) => {
    // Simplified color temperature conversion
    if (kelvin < 3000) return '#ff8c00';
    if (kelvin < 4000) return '#ffd700';
    if (kelvin < 5000) return '#ffffe0';
    if (kelvin < 6000) return '#ffffff';
    if (kelvin < 7000) return '#e6f3ff';
    return '#b3d9ff';
  };

  return (
    <div class="h-full flex flex-col bg-base-100">
      {/* Lighting Header */}
      <div class="flex items-center justify-between p-3 border-b border-base-300">
        <div class="flex items-center space-x-2">
          <Sun class="w-4 h-4 text-yellow-500" />
          <span class="text-sm font-medium text-base-content">Lighting</span>
        </div>
        
        <div class="flex items-center space-x-1">
          <div class="dropdown dropdown-end">
            <button class="btn btn-xs btn-primary" title="Add Light">
              <Plus class="w-3 h-3" />
            </button>
            <ul class="dropdown-content menu p-2 shadow bg-base-200 rounded-box w-32 text-xs">
              <For each={lightTypes}>
                {(type) => (
                  <li>
                    <button onClick={() => addLight(type.id)}>
                      <span class="mr-2">{type.icon}</span>
                      {type.name}
                    </button>
                  </li>
                )}
              </For>
            </ul>
          </div>
          <button class="btn btn-xs btn-ghost" title="Settings">
            <Settings class="w-3 h-3" />
          </button>
        </div>
      </div>

      <div class="flex-1 flex">
        {/* Lights List */}
        <div class="w-56 border-r border-base-300 flex flex-col">
          {/* Environment */}
          <div class="p-3 border-b border-base-300">
            <div class="text-xs text-base-content/60 uppercase tracking-wide mb-2">Environment</div>
            <div class="space-y-2">
              <div class="flex items-center justify-between">
                <span class="text-xs">HDRI</span>
                <button
                  class={`btn btn-xs ${environment().visible ? 'btn-primary' : 'btn-ghost'}`}
                  onClick={() => setEnvironment(prev => ({ ...prev, visible: !prev.visible }))}
                >
                  {environment().visible ? <Eye class="w-3 h-3" /> : <EyeOff class="w-3 h-3" />}
                </button>
              </div>
              <select class="select select-xs select-bordered w-full text-xs">
                <option>studio_small_04.hdr</option>
                <option>forest_slope.hdr</option>
                <option>sunset_jhbcentral.hdr</option>
                <option>kloppenheim_02.hdr</option>
              </select>
              <div class="flex items-center space-x-2">
                <span class="text-xs text-base-content/60">Intensity</span>
                <input
                  type="range"
                  min="0"
                  max="3"
                  step="0.1"
                  value={environment().intensity}
                  class="range range-xs flex-1"
                />
              </div>
            </div>
          </div>

          {/* Lights */}
          <div class="flex-1 overflow-y-auto">
            <div class="p-2">
              <div class="text-xs text-base-content/60 uppercase tracking-wide mb-2">Lights</div>
              <div class="space-y-1">
                <For each={lights()}>
                  {(light) => (
                    <div
                      class={`p-2 cursor-pointer hover:bg-base-200 rounded border-l-2 group ${
                        light.selected ? 'bg-base-200 border-primary' : 'border-transparent'
                      }`}
                      onClick={() => selectLight(light)}
                    >
                      <div class="flex items-center justify-between">
                        <div class="flex items-center space-x-2 flex-1 min-w-0">
                          <button
                            class={`w-3 h-3 rounded-full flex-shrink-0 ${
                              light.enabled ? 'bg-success' : 'bg-base-300'
                            }`}
                            onClick={(e) => {
                              e.stopPropagation();
                              toggleLight(light.id);
                            }}
                          />
                          <span class="text-xs">{getLightIcon(light.type)}</span>
                          <span class="text-xs font-medium truncate">{light.name}</span>
                        </div>
                        
                        <div class="flex space-x-1 opacity-0 group-hover:opacity-100">
                          <button
                            class="btn btn-xs btn-ghost p-0 w-4 h-4"
                            onClick={(e) => {
                              e.stopPropagation();
                              duplicateLight(light);
                            }}
                          >
                            <Copy class="w-2 h-2" />
                          </button>
                          <button
                            class="btn btn-xs btn-ghost p-0 w-4 h-4 text-error"
                            onClick={(e) => {
                              e.stopPropagation();
                              deleteLight(light.id);
                            }}
                          >
                            <Trash class="w-2 h-2" />
                          </button>
                        </div>
                      </div>
                      
                      <div class="flex items-center justify-between mt-1">
                        <div class="flex items-center space-x-2">
                          <div
                            class="w-2 h-2 rounded-full border"
                            style={{ 'background-color': light.color }}
                          />
                          <span class={`text-[10px] font-mono ${getIntensityColor(light.intensity)}`}>
                            {light.intensity.toFixed(1)}
                          </span>
                        </div>
                        <div class="flex items-center space-x-1">
                          <span class="text-[10px] text-base-content/40">{light.temperature}K</span>
                          {light.shadows && <span class="text-[10px] text-base-content/60">🌑</span>}
                        </div>
                      </div>
                      
                      <div class="text-[10px] text-base-content/40 mt-1 capitalize">
                        {light.type} light
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </div>
        </div>

        {/* Properties Panel */}
        <div class="flex-1 flex flex-col">
          {selectedLight() ? (
            <>
              {/* Light Info */}
              <div class="p-3 border-b border-base-300">
                <div class="flex items-center justify-between mb-2">
                  <h3 class="text-sm font-medium flex items-center">
                    <span class="mr-2">{getLightIcon(selectedLight().type)}</span>
                    {selectedLight().name}
                  </h3>
                  <button
                    class={`btn btn-xs ${selectedLight().enabled ? 'btn-success' : 'btn-ghost'}`}
                    onClick={() => toggleLight(selectedLight().id)}
                  >
                    {selectedLight().enabled ? <Eye class="w-3 h-3" /> : <EyeOff class="w-3 h-3" />}
                  </button>
                </div>
                <p class="text-xs text-base-content/60 capitalize">
                  {selectedLight().type} light
                </p>
              </div>

              {/* Properties */}
              <div class="flex-1 overflow-y-auto p-3 space-y-4">
                {/* Basic Properties */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Light Properties</h4>
                  <div class="space-y-2">
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Intensity</label>
                      <input
                        type="number"
                        step="0.1"
                        min="0"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={selectedLight().intensity}
                      />
                    </div>
                    <input
                      type="range"
                      min="0"
                      max="20"
                      step="0.1"
                      value={selectedLight().intensity}
                      class="range range-xs range-primary"
                    />
                  </div>
                </div>

                {/* Color & Temperature */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Color</h4>
                  <div class="space-y-2">
                    <div class="flex items-center space-x-2">
                      <input
                        type="color"
                        value={selectedLight().color}
                        class="w-8 h-6 rounded border-2 border-base-300"
                      />
                      <input
                        type="text"
                        value={selectedLight().color}
                        class="input input-xs input-bordered flex-1 text-xs font-mono"
                      />
                    </div>
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Temperature</label>
                      <input
                        type="number"
                        min="1000"
                        max="12000"
                        step="100"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={selectedLight().temperature}
                      />
                    </div>
                    <input
                      type="range"
                      min="1000"
                      max="12000"
                      step="100"
                      value={selectedLight().temperature}
                      class="range range-xs"
                    />
                    <div class="flex justify-between text-[10px] text-base-content/40">
                      <span>Warm</span>
                      <span>Cool</span>
                    </div>
                  </div>
                </div>

                {/* Position */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Position</h4>
                  <div class="grid grid-cols-3 gap-2">
                    <div>
                      <label class="text-[10px] text-base-content/60">X</label>
                      <input
                        type="number"
                        step="0.1"
                        class="input input-xs input-bordered w-full text-xs"
                        value={selectedLight().position.x}
                      />
                    </div>
                    <div>
                      <label class="text-[10px] text-base-content/60">Y</label>
                      <input
                        type="number"
                        step="0.1"
                        class="input input-xs input-bordered w-full text-xs"
                        value={selectedLight().position.y}
                      />
                    </div>
                    <div>
                      <label class="text-[10px] text-base-content/60">Z</label>
                      <input
                        type="number"
                        step="0.1"
                        class="input input-xs input-bordered w-full text-xs"
                        value={selectedLight().position.z}
                      />
                    </div>
                  </div>
                </div>

                {/* Rotation */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Rotation</h4>
                  <div class="grid grid-cols-3 gap-2">
                    <div>
                      <label class="text-[10px] text-base-content/60">X</label>
                      <input
                        type="number"
                        step="1"
                        class="input input-xs input-bordered w-full text-xs"
                        value={selectedLight().rotation.x}
                      />
                    </div>
                    <div>
                      <label class="text-[10px] text-base-content/60">Y</label>
                      <input
                        type="number"
                        step="1"
                        class="input input-xs input-bordered w-full text-xs"
                        value={selectedLight().rotation.y}
                      />
                    </div>
                    <div>
                      <label class="text-[10px] text-base-content/60">Z</label>
                      <input
                        type="number"
                        step="1"
                        class="input input-xs input-bordered w-full text-xs"
                        value={selectedLight().rotation.z}
                      />
                    </div>
                  </div>
                </div>

                {/* Shadows */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Shadows</h4>
                  <div class="space-y-2">
                    <label class="flex items-center space-x-2 cursor-pointer">
                      <input 
                        type="checkbox" 
                        class="checkbox checkbox-xs"
                        checked={selectedLight().shadows}
                      />
                      <span class="text-xs text-base-content/60">Cast Shadows</span>
                    </label>
                    {selectedLight().shadows && (
                      <>
                        <div class="flex items-center justify-between">
                          <label class="text-xs text-base-content/60">Shadow Bias</label>
                          <input
                            type="number"
                            step="0.001"
                            min="0"
                            class="input input-xs input-bordered w-16 text-xs"
                            value={0.005}
                          />
                        </div>
                        <div class="flex items-center justify-between">
                          <label class="text-xs text-base-content/60">Shadow Blur</label>
                          <input
                            type="number"
                            step="0.1"
                            min="0"
                            class="input input-xs input-bordered w-16 text-xs"
                            value={1.0}
                          />
                        </div>
                      </>
                    )}
                  </div>
                </div>

                {/* Type-specific properties */}
                {(selectedLight().type === 'spot' || selectedLight().type === 'area') && (
                  <div class="space-y-2">
                    <h4 class="text-xs font-medium text-base-content/80">Shape</h4>
                    <div class="space-y-2">
                      {selectedLight().type === 'spot' && (
                        <>
                          <div class="flex items-center justify-between">
                            <label class="text-xs text-base-content/60">Cone Angle</label>
                            <input
                              type="number"
                              min="1"
                              max="180"
                              class="input input-xs input-bordered w-16 text-xs"
                              value={45}
                            />
                          </div>
                          <div class="flex items-center justify-between">
                            <label class="text-xs text-base-content/60">Falloff</label>
                            <input
                              type="number"
                              step="0.1"
                              min="0"
                              max="1"
                              class="input input-xs input-bordered w-16 text-xs"
                              value={0.5}
                            />
                          </div>
                        </>
                      )}
                      {selectedLight().type === 'area' && (
                        <>
                          <div class="flex items-center justify-between">
                            <label class="text-xs text-base-content/60">Width</label>
                            <input
                              type="number"
                              step="0.1"
                              min="0.1"
                              class="input input-xs input-bordered w-16 text-xs"
                              value={2.0}
                            />
                          </div>
                          <div class="flex items-center justify-between">
                            <label class="text-xs text-base-content/60">Height</label>
                            <input
                              type="number"
                              step="0.1"
                              min="0.1"
                              class="input input-xs input-bordered w-16 text-xs"
                              value={2.0}
                            />
                          </div>
                        </>
                      )}
                    </div>
                  </div>
                )}
              </div>
            </>
          ) : (
            <div class="flex-1 flex flex-col">
              {/* Global Settings */}
              <div class="p-3 border-b border-base-300">
                <h3 class="text-sm font-medium">Global Lighting</h3>
              </div>
              
              <div class="flex-1 overflow-y-auto p-3 space-y-4">
                <div class="space-y-2">
                  <div class="flex items-center justify-between">
                    <label class="text-xs text-base-content/60">Exposure</label>
                    <input
                      type="number"
                      step="0.1"
                      class="input input-xs input-bordered w-16 text-xs"
                      value={globalSettings().exposure}
                    />
                  </div>
                  <div class="flex items-center justify-between">
                    <label class="text-xs text-base-content/60">Gamma</label>
                    <input
                      type="number"
                      step="0.1"
                      class="input input-xs input-bordered w-16 text-xs"
                      value={globalSettings().gamma}
                    />
                  </div>
                  <label class="flex items-center space-x-2 cursor-pointer">
                    <input 
                      type="checkbox" 
                      class="checkbox checkbox-xs"
                      checked={globalSettings().shadows}
                    />
                    <span class="text-xs text-base-content/60">Global Shadows</span>
                  </label>
                  <label class="flex items-center space-x-2 cursor-pointer">
                    <input 
                      type="checkbox" 
                      class="checkbox checkbox-xs"
                      checked={globalSettings().ambientOcclusion}
                    />
                    <span class="text-xs text-base-content/60">Ambient Occlusion</span>
                  </label>
                  <label class="flex items-center space-x-2 cursor-pointer">
                    <input 
                      type="checkbox" 
                      class="checkbox checkbox-xs"
                      checked={globalSettings().volumetrics}
                    />
                    <span class="text-xs text-base-content/60">Volumetrics</span>
                  </label>
                </div>
                
                <div class="text-center text-base-content/40">
                  <Sun class="w-8 h-8 mx-auto mb-2 text-yellow-500" />
                  <p class="text-xs">
                    {lights().filter(l => l.enabled).length} of {lights().length} lights enabled
                  </p>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default Lighting;