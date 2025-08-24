import { createSignal, For } from 'solid-js';
import { 
  Play, Stop, Reset, ArrowLeft, ArrowRight, Plus, 
  Settings, Copy, Trash, Edit 
} from '@/ui/icons';

function Animation() {
  const [isPlaying, setIsPlaying] = createSignal(false);
  const [currentFrame, setCurrentFrame] = createSignal(30);
  const [totalFrames, setTotalFrames] = createSignal(120);
  const [fps, setFps] = createSignal(24);
  
  const [animations, setAnimations] = createSignal([
    {
      id: 1,
      name: 'Idle Animation',
      duration: 2.5,
      frames: 60,
      loop: true,
      selected: true
    },
    {
      id: 2,
      name: 'Walk Cycle',
      duration: 1.0,
      frames: 24,
      loop: true,
      selected: false
    }
  ]);

  const [keyframes, setKeyframes] = createSignal([
    { id: 1, frame: 0, property: 'position.x', value: 0, selected: false },
    { id: 2, frame: 30, property: 'position.x', value: 5, selected: false },
    { id: 3, frame: 60, property: 'position.x', value: 0, selected: false },
    { id: 4, frame: 0, property: 'rotation.y', value: 0, selected: false },
    { id: 5, frame: 60, property: 'rotation.y', value: 360, selected: false },
  ]);

  const [selectedAnimation, setSelectedAnimation] = createSignal(animations()[0]);

  const togglePlayback = () => {
    setIsPlaying(!isPlaying());
  };

  const goToFrame = (frame) => {
    setCurrentFrame(Math.max(0, Math.min(frame, totalFrames())));
  };

  const selectAnimation = (animation) => {
    setAnimations(prev => prev.map(a => ({
      ...a,
      selected: a.id === animation.id
    })));
    setSelectedAnimation(animation);
  };

  const getTimeFromFrame = (frame) => {
    return (frame / fps()).toFixed(2);
  };

  const getFrameFromTime = (time) => {
    return Math.round(time * fps());
  };

  const progress = () => (currentFrame() / totalFrames()) * 100;

  return (
    <div class="h-full flex flex-col bg-base-100">
      {/* Animation Header */}
      <div class="flex items-center justify-between p-3 border-b border-base-300">
        <div class="flex items-center space-x-2">
          <div class="w-4 h-4 bg-gradient-to-r from-green-400 to-blue-500 rounded-full"></div>
          <span class="text-sm font-medium text-base-content">Animation</span>
        </div>
        
        <div class="flex items-center space-x-1">
          <button class="btn btn-xs btn-primary" title="Add Animation">
            <Plus class="w-3 h-3" />
          </button>
          <button class="btn btn-xs btn-ghost" title="Settings">
            <Settings class="w-3 h-3" />
          </button>
        </div>
      </div>

      {/* Timeline Controls */}
      <div class="p-3 border-b border-base-300 space-y-2">
        <div class="flex items-center justify-between">
          <div class="flex items-center space-x-2">
            <button
              class="btn btn-xs btn-ghost"
              onClick={() => goToFrame(0)}
              title="Go to Start"
            >
              <ArrowLeft class="w-3 h-3" />
              <ArrowLeft class="w-3 h-3 -ml-2" />
            </button>
            
            <button
              class={`btn btn-xs ${isPlaying() ? 'btn-warning' : 'btn-success'}`}
              onClick={togglePlayback}
            >
              {isPlaying() ? <Stop class="w-3 h-3" /> : <Play class="w-3 h-3" />}
            </button>
            
            <button
              class="btn btn-xs btn-ghost"
              onClick={() => goToFrame(totalFrames())}
              title="Go to End"
            >
              <ArrowRight class="w-3 h-3" />
              <ArrowRight class="w-3 h-3 -ml-2" />
            </button>
            
            <button
              class="btn btn-xs btn-ghost"
              onClick={() => {
                setIsPlaying(false);
                setCurrentFrame(0);
              }}
              title="Reset"
            >
              <Reset class="w-3 h-3" />
            </button>
          </div>
          
          <div class="flex items-center space-x-2 text-xs">
            <span class="text-base-content/60">Frame:</span>
            <input
              type="number"
              class="input input-xs input-bordered w-16"
              value={currentFrame()}
              onChange={(e) => goToFrame(parseInt(e.target.value) || 0)}
            />
            <span class="text-base-content/40">/ {totalFrames()}</span>
            <span class="text-base-content/60 ml-4">FPS:</span>
            <input
              type="number"
              class="input input-xs input-bordered w-12"
              value={fps()}
              onChange={(e) => setFps(parseInt(e.target.value) || 24)}
            />
          </div>
        </div>
        
        {/* Timeline Scrubber */}
        <div class="relative">
          <div class="h-6 bg-base-200 rounded relative overflow-hidden">
            <div
              class="absolute top-0 left-0 h-full bg-primary/20"
              style={{ width: `${progress()}%` }}
            />
            <input
              type="range"
              min="0"
              max={totalFrames()}
              value={currentFrame()}
              onChange={(e) => setCurrentFrame(parseInt(e.target.value))}
              class="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
            />
            <div
              class="absolute top-0 w-0.5 h-full bg-primary"
              style={{ left: `${progress()}%` }}
            />
          </div>
          <div class="flex justify-between text-[10px] text-base-content/40 mt-1">
            <span>0s</span>
            <span>{getTimeFromFrame(totalFrames())}s</span>
          </div>
        </div>
      </div>

      <div class="flex-1 flex">
        {/* Animations List */}
        <div class="w-48 border-r border-base-300 flex flex-col">
          <div class="p-2 border-b border-base-300 flex items-center justify-between">
            <div class="text-xs text-base-content/60 uppercase tracking-wide">Animations</div>
          </div>
          
          <div class="flex-1 overflow-y-auto">
            <For each={animations()}>
              {(animation) => (
                <div
                  class={`p-2 cursor-pointer hover:bg-base-200 border-l-2 ${
                    animation.selected ? 'bg-base-200 border-primary' : 'border-transparent'
                  }`}
                  onClick={() => selectAnimation(animation)}
                >
                  <div class="flex items-center justify-between">
                    <div class="flex items-center space-x-2 flex-1 min-w-0">
                      <div class="w-3 h-3 bg-gradient-to-r from-green-400 to-blue-500 rounded-full flex-shrink-0"></div>
                      <span class="text-xs truncate">{animation.name}</span>
                    </div>
                    
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
                  
                  <div class="text-[10px] text-base-content/40 mt-1">
                    {animation.duration}s • {animation.frames} frames
                  </div>
                  
                  {animation.loop && (
                    <div class="text-[10px] text-primary mt-1">
                      ↻ Loop
                    </div>
                  )}
                </div>
              )}
            </For>
          </div>
        </div>

        {/* Keyframes Panel */}
        <div class="flex-1 flex flex-col">
          <div class="p-3 border-b border-base-300">
            <div class="flex items-center justify-between">
              <h3 class="text-sm font-medium">Keyframes</h3>
              <button class="btn btn-xs btn-primary" title="Add Keyframe">
                <Plus class="w-3 h-3" />
              </button>
            </div>
          </div>

          <div class="flex-1 overflow-y-auto">
            <div class="space-y-1">
              <For each={keyframes()}>
                {(keyframe) => (
                  <div
                    class={`flex items-center justify-between p-2 hover:bg-base-200 cursor-pointer ${
                      keyframe.selected ? 'bg-base-200 border-l-2 border-primary' : ''
                    }`}
                    onClick={() => {
                      setKeyframes(prev => prev.map(k => ({
                        ...k,
                        selected: k.id === keyframe.id
                      })));
                    }}
                  >
                    <div class="flex-1 min-w-0">
                      <div class="flex items-center space-x-2">
                        <div class="w-2 h-2 bg-primary rounded-full"></div>
                        <span class="text-xs font-mono">{keyframe.property}</span>
                      </div>
                      <div class="text-[10px] text-base-content/40 mt-1">
                        Frame {keyframe.frame} • {getTimeFromFrame(keyframe.frame)}s
                      </div>
                    </div>
                    
                    <div class="text-xs text-base-content/60 font-mono">
                      {keyframe.value}
                    </div>
                    
                    <div class="flex space-x-1 ml-2">
                      <button class="btn btn-xs btn-ghost p-0 w-4 h-4">
                        <Edit class="w-2 h-2" />
                      </button>
                      <button class="btn btn-xs btn-ghost p-0 w-4 h-4 text-error">
                        <Trash class="w-2 h-2" />
                      </button>
                    </div>
                  </div>
                )}
              </For>
            </div>
            
            {keyframes().length === 0 && (
              <div class="flex-1 flex items-center justify-center">
                <div class="text-center text-base-content/40">
                  <div class="w-8 h-8 bg-gradient-to-r from-green-400 to-blue-500 rounded-full mx-auto mb-2"></div>
                  <p class="text-xs">No keyframes</p>
                  <p class="text-xs">Add keyframes to animate properties</p>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export default Animation;