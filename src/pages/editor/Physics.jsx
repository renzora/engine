import { createSignal, For } from 'solid-js';
import { Play, Stop, Reset, Settings, Plus, Trash, Box } from '@/ui/icons';

function Physics() {
  const [isSimulating, setIsSimulating] = createSignal(false);
  const [bodies, setBodies] = createSignal([
    {
      id: 1,
      name: 'Ground Plane',
      type: 'static',
      shape: 'plane',
      mass: 0,
      selected: true
    },
    {
      id: 2,
      name: 'Falling Cube',
      type: 'dynamic',
      shape: 'box',
      mass: 1.0,
      selected: false
    },
    {
      id: 3,
      name: 'Ball',
      type: 'dynamic',
      shape: 'sphere',
      mass: 0.5,
      selected: false
    }
  ]);
  
  const [selectedBody, setSelectedBody] = createSignal(bodies()[0]);
  const [worldSettings, setWorldSettings] = createSignal({
    gravity: -9.81,
    timeStep: 1/60,
    iterations: 10
  });

  const selectBody = (body) => {
    setBodies(prev => prev.map(b => ({
      ...b,
      selected: b.id === body.id
    })));
    setSelectedBody(body);
  };

  const toggleSimulation = () => {
    setIsSimulating(!isSimulating());
  };

  const resetSimulation = () => {
    setIsSimulating(false);
    // Reset physics bodies to initial positions
  };

  const getShapeIcon = (shape) => {
    switch (shape) {
      case 'box': return '📦';
      case 'sphere': return '⚪';
      case 'capsule': return '💊';
      case 'plane': return '📐';
      default: return '📦';
    }
  };

  const getTypeColor = (type) => {
    switch (type) {
      case 'static': return 'text-base-content/60';
      case 'dynamic': return 'text-primary';
      case 'kinematic': return 'text-secondary';
      default: return 'text-base-content';
    }
  };

  return (
    <div class="h-full flex flex-col bg-base-100">
      {/* Physics Header */}
      <div class="flex items-center justify-between p-3 border-b border-base-300">
        <div class="flex items-center space-x-2">
          <div class="w-4 h-4 bg-gradient-to-r from-blue-400 to-purple-500 rounded-full"></div>
          <span class="text-sm font-medium text-base-content">Physics</span>
        </div>
        
        <div class="flex items-center space-x-1">
          <button
            class={`btn btn-xs ${isSimulating() ? 'btn-warning' : 'btn-success'}`}
            onClick={toggleSimulation}
            title={isSimulating() ? 'Stop Simulation' : 'Start Simulation'}
          >
            {isSimulating() ? <Stop class="w-3 h-3" /> : <Play class="w-3 h-3" />}
          </button>
          <button class="btn btn-xs btn-ghost" onClick={resetSimulation} title="Reset">
            <Reset class="w-3 h-3" />
          </button>
          <button class="btn btn-xs btn-ghost" title="Settings">
            <Settings class="w-3 h-3" />
          </button>
        </div>
      </div>

      <div class="flex-1 flex">
        {/* Bodies List */}
        <div class="w-48 border-r border-base-300 flex flex-col">
          <div class="p-2 border-b border-base-300 flex items-center justify-between">
            <div class="text-xs text-base-content/60 uppercase tracking-wide">Bodies</div>
            <button class="btn btn-xs btn-ghost" title="Add Body">
              <Plus class="w-3 h-3" />
            </button>
          </div>
          
          <div class="flex-1 overflow-y-auto">
            <For each={bodies()}>
              {(body) => (
                <div
                  class={`p-2 cursor-pointer hover:bg-base-200 border-l-2 ${
                    body.selected ? 'bg-base-200 border-primary' : 'border-transparent'
                  }`}
                  onClick={() => selectBody(body)}
                >
                  <div class="flex items-center justify-between">
                    <div class="flex items-center space-x-2 flex-1 min-w-0">
                      <span class="flex-shrink-0 text-sm">
                        {getShapeIcon(body.shape)}
                      </span>
                      <span class="text-xs truncate">{body.name}</span>
                    </div>
                    
                    <button
                      class="btn btn-xs btn-ghost p-0 w-4 h-4 text-error opacity-0 group-hover:opacity-100"
                      onClick={(e) => {
                        e.stopPropagation();
                        setBodies(prev => prev.filter(b => b.id !== body.id));
                      }}
                    >
                      <Trash class="w-2 h-2" />
                    </button>
                  </div>
                  
                  <div class="flex items-center justify-between mt-1">
                    <span class={`text-[10px] capitalize ${getTypeColor(body.type)}`}>
                      {body.type}
                    </span>
                    <span class="text-[10px] text-base-content/40">
                      {body.mass}kg
                    </span>
                  </div>
                </div>
              )}
            </For>
          </div>
        </div>

        {/* Properties Panel */}
        <div class="flex-1 flex flex-col">
          {selectedBody() ? (
            <>
              {/* Body Info */}
              <div class="p-3 border-b border-base-300">
                <h3 class="text-sm font-medium flex items-center">
                  <span class="mr-2">{getShapeIcon(selectedBody().shape)}</span>
                  {selectedBody().name}
                </h3>
                <p class="text-xs text-base-content/60 mt-1 capitalize">
                  {selectedBody().type} {selectedBody().shape}
                </p>
              </div>

              {/* Properties */}
              <div class="flex-1 overflow-y-auto p-3 space-y-4">
                {/* Type & Shape */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Type & Shape</h4>
                  <div class="space-y-2">
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Body Type</label>
                      <select class="select select-xs select-bordered text-xs">
                        <option value="static" selected={selectedBody().type === 'static'}>Static</option>
                        <option value="dynamic" selected={selectedBody().type === 'dynamic'}>Dynamic</option>
                        <option value="kinematic" selected={selectedBody().type === 'kinematic'}>Kinematic</option>
                      </select>
                    </div>
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Shape</label>
                      <select class="select select-xs select-bordered text-xs">
                        <option value="box" selected={selectedBody().shape === 'box'}>Box</option>
                        <option value="sphere" selected={selectedBody().shape === 'sphere'}>Sphere</option>
                        <option value="capsule" selected={selectedBody().shape === 'capsule'}>Capsule</option>
                        <option value="plane" selected={selectedBody().shape === 'plane'}>Plane</option>
                        <option value="mesh">Mesh</option>
                      </select>
                    </div>
                  </div>
                </div>

                {/* Physical Properties */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Physical Properties</h4>
                  <div class="space-y-2">
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Mass (kg)</label>
                      <input
                        type="number"
                        step="0.1"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={selectedBody().mass}
                      />
                    </div>
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Friction</label>
                      <input
                        type="number"
                        step="0.1"
                        min="0"
                        max="1"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={0.5}
                      />
                    </div>
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Restitution</label>
                      <input
                        type="number"
                        step="0.1"
                        min="0"
                        max="1"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={0.3}
                      />
                    </div>
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Linear Damping</label>
                      <input
                        type="number"
                        step="0.01"
                        min="0"
                        max="1"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={0.1}
                      />
                    </div>
                    <div class="flex items-center justify-between">
                      <label class="text-xs text-base-content/60">Angular Damping</label>
                      <input
                        type="number"
                        step="0.01"
                        min="0"
                        max="1"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={0.1}
                      />
                    </div>
                  </div>
                </div>

                {/* Constraints */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Constraints</h4>
                  <div class="space-y-2">
                    <label class="flex items-center space-x-2 cursor-pointer">
                      <input type="checkbox" class="checkbox checkbox-xs" />
                      <span class="text-xs text-base-content/60">Freeze Position X</span>
                    </label>
                    <label class="flex items-center space-x-2 cursor-pointer">
                      <input type="checkbox" class="checkbox checkbox-xs" />
                      <span class="text-xs text-base-content/60">Freeze Position Y</span>
                    </label>
                    <label class="flex items-center space-x-2 cursor-pointer">
                      <input type="checkbox" class="checkbox checkbox-xs" />
                      <span class="text-xs text-base-content/60">Freeze Position Z</span>
                    </label>
                    <label class="flex items-center space-x-2 cursor-pointer">
                      <input type="checkbox" class="checkbox checkbox-xs" />
                      <span class="text-xs text-base-content/60">Freeze Rotation</span>
                    </label>
                  </div>
                </div>
              </div>
            </>
          ) : (
            <div class="flex-1 flex flex-col">
              {/* World Settings */}
              <div class="p-3 border-b border-base-300">
                <h3 class="text-sm font-medium">World Settings</h3>
              </div>
              
              <div class="flex-1 overflow-y-auto p-3 space-y-4">
                <div class="space-y-2">
                  <div class="flex items-center justify-between">
                    <label class="text-xs text-base-content/60">Gravity</label>
                    <input
                      type="number"
                      step="0.1"
                      class="input input-xs input-bordered w-16 text-xs"
                      value={worldSettings().gravity}
                    />
                  </div>
                  <div class="flex items-center justify-between">
                    <label class="text-xs text-base-content/60">Time Step</label>
                    <input
                      type="number"
                      step="0.001"
                      class="input input-xs input-bordered w-16 text-xs"
                      value={worldSettings().timeStep}
                    />
                  </div>
                  <div class="flex items-center justify-between">
                    <label class="text-xs text-base-content/60">Iterations</label>
                    <input
                      type="number"
                      class="input input-xs input-bordered w-16 text-xs"
                      value={worldSettings().iterations}
                    />
                  </div>
                </div>
                
                <div class="pt-4 text-center text-base-content/40">
                  <div class="w-8 h-8 bg-gradient-to-r from-blue-400 to-purple-500 rounded-full mx-auto mb-2"></div>
                  <p class="text-xs">
                    {isSimulating() ? 'Physics simulation running' : 'Physics simulation paused'}
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

export default Physics;