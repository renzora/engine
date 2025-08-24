import { createSignal, For } from 'solid-js';
import { 
  Plus, Settings, Play, Stop, Reset, Copy, Trash, 
  ArrowUp, ArrowDown, Grid, Maximize 
} from '@/ui/icons';

function Particles() {
  const [systems, setSystems] = createSignal([
    {
      id: 1,
      name: 'Fire Emitter',
      enabled: true,
      particles: 125,
      emission: 50,
      lifetime: 2.5,
      selected: true
    },
    {
      id: 2,
      name: 'Smoke Trail',
      enabled: false,
      particles: 85,
      emission: 25,
      lifetime: 4.0,
      selected: false
    }
  ]);

  const [selectedSystem, setSelectedSystem] = createSignal(systems()[0]);
  
  const selectSystem = (system) => {
    setSystems(prev => prev.map(s => ({
      ...s,
      selected: s.id === system.id
    })));
    setSelectedSystem(system);
  };

  const toggleSystem = (systemId) => {
    setSystems(prev => prev.map(s => 
      s.id === systemId ? { ...s, enabled: !s.enabled } : s
    ));
  };

  const deleteSystem = (systemId) => {
    setSystems(prev => prev.filter(s => s.id !== systemId));
  };

  const duplicateSystem = (system) => {
    const newSystem = {
      ...system,
      id: Date.now(),
      name: `${system.name} Copy`,
      selected: false
    };
    setSystems(prev => [...prev, newSystem]);
  };

  return (
    <div class="h-full flex flex-col bg-base-100">
      {/* Particles Header */}
      <div class="flex items-center justify-between p-3 border-b border-base-300">
        <div class="flex items-center space-x-2">
          <div class="w-4 h-4 bg-gradient-to-r from-orange-400 to-red-500 rounded-full"></div>
          <span class="text-sm font-medium text-base-content">Particle Systems</span>
        </div>
        
        <div class="flex items-center space-x-1">
          <button class="btn btn-xs btn-primary" title="Add System">
            <Plus class="w-3 h-3" />
          </button>
          <button class="btn btn-xs btn-ghost" title="Settings">
            <Settings class="w-3 h-3" />
          </button>
        </div>
      </div>

      <div class="flex-1 flex">
        {/* Systems List */}
        <div class="w-48 border-r border-base-300 flex flex-col">
          <div class="p-2 border-b border-base-300">
            <div class="text-xs text-base-content/60 uppercase tracking-wide">Systems</div>
          </div>
          
          <div class="flex-1 overflow-y-auto">
            <For each={systems()}>
              {(system) => (
                <div
                  class={`p-2 cursor-pointer hover:bg-base-200 border-l-2 ${
                    system.selected ? 'bg-base-200 border-primary' : 'border-transparent'
                  }`}
                  onClick={() => selectSystem(system)}
                >
                  <div class="flex items-center justify-between">
                    <div class="flex items-center space-x-2 flex-1 min-w-0">
                      <button
                        class={`w-3 h-3 rounded-full flex-shrink-0 ${
                          system.enabled ? 'bg-success' : 'bg-base-300'
                        }`}
                        onClick={(e) => {
                          e.stopPropagation();
                          toggleSystem(system.id);
                        }}
                      />
                      <span class="text-xs truncate">{system.name}</span>
                    </div>
                    
                    <div class="flex space-x-1">
                      <button
                        class="btn btn-xs btn-ghost p-0 w-4 h-4"
                        onClick={(e) => {
                          e.stopPropagation();
                          duplicateSystem(system);
                        }}
                      >
                        <Copy class="w-2 h-2" />
                      </button>
                      <button
                        class="btn btn-xs btn-ghost p-0 w-4 h-4 text-error"
                        onClick={(e) => {
                          e.stopPropagation();
                          deleteSystem(system.id);
                        }}
                      >
                        <Trash class="w-2 h-2" />
                      </button>
                    </div>
                  </div>
                  
                  <div class="text-[10px] text-base-content/40 mt-1">
                    {system.particles} particles • {system.emission}/s
                  </div>
                </div>
              )}
            </For>
          </div>
        </div>

        {/* Properties Panel */}
        <div class="flex-1 flex flex-col">
          {selectedSystem() ? (
            <>
              {/* System Controls */}
              <div class="p-3 border-b border-base-300">
                <div class="flex items-center justify-between mb-2">
                  <h3 class="text-sm font-medium">{selectedSystem().name}</h3>
                  <div class="flex space-x-1">
                    <button class="btn btn-xs btn-success">
                      <Play class="w-3 h-3" />
                    </button>
                    <button class="btn btn-xs btn-warning">
                      <Stop class="w-3 h-3" />
                    </button>
                    <button class="btn btn-xs btn-ghost">
                      <Reset class="w-3 h-3" />
                    </button>
                  </div>
                </div>
              </div>

              {/* Properties */}
              <div class="flex-1 overflow-y-auto p-3 space-y-4">
                {/* Emission Properties */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Emission</h4>
                  <div class="space-y-2">
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Rate</label>
                      <input
                        type="number"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={selectedSystem().emission}
                      />
                    </div>
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Burst</label>
                      <input
                        type="number"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={0}
                      />
                    </div>
                  </div>
                </div>

                {/* Particle Properties */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Particles</h4>
                  <div class="space-y-2">
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Max Count</label>
                      <input
                        type="number"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={selectedSystem().particles}
                      />
                    </div>
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Lifetime</label>
                      <input
                        type="number"
                        step="0.1"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={selectedSystem().lifetime}
                      />
                    </div>
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Size</label>
                      <input
                        type="number"
                        step="0.1"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={1.0}
                      />
                    </div>
                  </div>
                </div>

                {/* Forces */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Forces</h4>
                  <div class="space-y-2">
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Gravity</label>
                      <input
                        type="number"
                        step="0.1"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={-9.8}
                      />
                    </div>
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Wind</label>
                      <input
                        type="number"
                        step="0.1"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={0}
                      />
                    </div>
                  </div>
                </div>

                {/* Rendering */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Rendering</h4>
                  <div class="space-y-2">
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Material</label>
                      <select class="select select-xs select-bordered text-xs">
                        <option>Default</option>
                        <option>Fire</option>
                        <option>Smoke</option>
                        <option>Sparks</option>
                      </select>
                    </div>
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Blend Mode</label>
                      <select class="select select-xs select-bordered text-xs">
                        <option>Alpha</option>
                        <option>Additive</option>
                        <option>Multiply</option>
                      </select>
                    </div>
                  </div>
                </div>
              </div>
            </>
          ) : (
            <div class="flex-1 flex items-center justify-center">
              <div class="text-center text-base-content/40">
                <div class="w-8 h-8 bg-gradient-to-r from-orange-400 to-red-500 rounded-full mx-auto mb-2"></div>
                <p class="text-xs">Select a particle system</p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default Particles;