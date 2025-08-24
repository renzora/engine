import { createSignal, For } from 'solid-js';
import { 
  Plus, Settings, Trash, Edit, Copy, Eye, EyeOff,
  ArrowUp, ArrowDown, Maximize, Reset 
} from '@/ui/icons';

function PostProcessing() {
  const [effects, setEffects] = createSignal([
    {
      id: 1,
      name: 'Tone Mapping',
      type: 'tonemapping',
      enabled: true,
      intensity: 1.0,
      order: 1,
      selected: true,
      settings: {
        type: 'ACES',
        exposure: 1.0,
        whitePoint: 1.0
      }
    },
    {
      id: 2,
      name: 'Bloom',
      type: 'bloom',
      enabled: true,
      intensity: 0.8,
      order: 2,
      selected: false,
      settings: {
        threshold: 1.0,
        radius: 0.5,
        strength: 1.2
      }
    },
    {
      id: 3,
      name: 'Depth of Field',
      type: 'dof',
      enabled: false,
      intensity: 1.0,
      order: 3,
      selected: false,
      settings: {
        focalDistance: 10.0,
        focalLength: 50,
        fStop: 2.8,
        bokehScale: 1.0
      }
    },
    {
      id: 4,
      name: 'Color Grading',
      type: 'colorgrading',
      enabled: true,
      intensity: 0.6,
      order: 4,
      selected: false,
      settings: {
        temperature: 0.0,
        tint: 0.0,
        saturation: 1.1,
        contrast: 1.05,
        brightness: 0.0,
        gamma: 1.0
      }
    },
    {
      id: 5,
      name: 'Vignette',
      type: 'vignette',
      enabled: false,
      intensity: 0.3,
      order: 5,
      selected: false,
      settings: {
        intensity: 0.3,
        smoothness: 0.5,
        roundness: 1.0
      }
    }
  ]);

  const [selectedEffect, setSelectedEffect] = createSignal(effects()[0]);
  const [previewMode, setPreviewMode] = createSignal('split'); // 'split', 'before', 'after'

  const availableEffects = [
    { type: 'bloom', name: 'Bloom', icon: '✨' },
    { type: 'dof', name: 'Depth of Field', icon: '📷' },
    { type: 'colorgrading', name: 'Color Grading', icon: '🎨' },
    { type: 'vignette', name: 'Vignette', icon: '⭕' },
    { type: 'chromaticaberration', name: 'Chromatic Aberration', icon: '🌈' },
    { type: 'filmgrain', name: 'Film Grain', icon: '📺' },
    { type: 'motionblur', name: 'Motion Blur', icon: '💨' },
    { type: 'ssao', name: 'Screen Space AO', icon: '🌑' },
    { type: 'ssr', name: 'Screen Space Reflections', icon: '🪞' },
    { type: 'fxaa', name: 'FXAA', icon: '🔧' }
  ];

  const selectEffect = (effect) => {
    setEffects(prev => prev.map(e => ({
      ...e,
      selected: e.id === effect.id
    })));
    setSelectedEffect(effect);
  };

  const toggleEffect = (effectId) => {
    setEffects(prev => prev.map(e => 
      e.id === effectId ? { ...e, enabled: !e.enabled } : e
    ));
  };

  const deleteEffect = (effectId) => {
    setEffects(prev => prev.filter(e => e.id !== effectId));
    if (selectedEffect()?.id === effectId) {
      setSelectedEffect(effects().length > 1 ? effects()[0] : null);
    }
  };

  const duplicateEffect = (effect) => {
    const newEffect = {
      ...effect,
      id: Date.now(),
      name: `${effect.name} Copy`,
      order: effects().length + 1,
      selected: false
    };
    setEffects(prev => [...prev, newEffect]);
  };

  const moveEffect = (effectId, direction) => {
    const currentIndex = effects().findIndex(e => e.id === effectId);
    const newIndex = direction === 'up' ? currentIndex - 1 : currentIndex + 1;
    
    if (newIndex >= 0 && newIndex < effects().length) {
      const newEffects = [...effects()];
      [newEffects[currentIndex], newEffects[newIndex]] = [newEffects[newIndex], newEffects[currentIndex]];
      
      // Update order values
      newEffects.forEach((effect, index) => {
        effect.order = index + 1;
      });
      
      setEffects(newEffects);
    }
  };

  const addEffect = (effectType) => {
    const effectTemplate = availableEffects.find(e => e.type === effectType);
    if (!effectTemplate) return;

    const newEffect = {
      id: Date.now(),
      name: effectTemplate.name,
      type: effectType,
      enabled: true,
      intensity: 1.0,
      order: effects().length + 1,
      selected: false,
      settings: getDefaultSettings(effectType)
    };

    setEffects(prev => [...prev, newEffect]);
  };

  const getDefaultSettings = (type) => {
    switch (type) {
      case 'bloom':
        return { threshold: 1.0, radius: 0.5, strength: 1.2 };
      case 'dof':
        return { focalDistance: 10.0, focalLength: 50, fStop: 2.8, bokehScale: 1.0 };
      case 'colorgrading':
        return { temperature: 0.0, tint: 0.0, saturation: 1.0, contrast: 1.0, brightness: 0.0, gamma: 1.0 };
      case 'vignette':
        return { intensity: 0.3, smoothness: 0.5, roundness: 1.0 };
      case 'chromaticaberration':
        return { intensity: 0.5, offset: 0.002 };
      case 'filmgrain':
        return { intensity: 0.1, size: 1.0 };
      default:
        return {};
    }
  };

  const getEffectIcon = (type) => {
    const effect = availableEffects.find(e => e.type === type);
    return effect ? effect.icon : '⚡';
  };

  const renderEffectSettings = (effect) => {
    if (!effect || !effect.settings) return null;

    const { settings } = effect;
    
    return (
      <div class="space-y-2">
        <For each={Object.entries(settings)}>
          {([key, value]) => (
            <div class="flex items-center justify-between">
              <label class="text-xs text-base-content/60 capitalize">
                {key.replace(/([A-Z])/g, ' $1').trim()}
              </label>
              {typeof value === 'boolean' ? (
                <input
                  type="checkbox"
                  class="checkbox checkbox-xs"
                  checked={value}
                />
              ) : typeof value === 'string' ? (
                <select class="select select-xs select-bordered text-xs">
                  <option selected>{value}</option>
                </select>
              ) : (
                <input
                  type="number"
                  step={key.includes('Distance') || key.includes('Length') ? '0.1' : '0.01'}
                  class="input input-xs input-bordered w-16 text-xs"
                  value={value}
                />
              )}
            </div>
          )}
        </For>
      </div>
    );
  };

  return (
    <div class="h-full flex flex-col bg-base-100">
      {/* Post Processing Header */}
      <div class="flex items-center justify-between p-3 border-b border-base-300">
        <div class="flex items-center space-x-2">
          <div class="w-4 h-4 bg-gradient-to-r from-pink-400 to-purple-500 rounded-full"></div>
          <span class="text-sm font-medium text-base-content">Post Processing</span>
        </div>
        
        <div class="flex items-center space-x-1">
          <div class="dropdown dropdown-end">
            <button class="btn btn-xs btn-primary" title="Add Effect">
              <Plus class="w-3 h-3" />
            </button>
            <ul class="dropdown-content menu p-2 shadow bg-base-200 rounded-box w-48 text-xs">
              <For each={availableEffects}>
                {(effect) => (
                  <li>
                    <button onClick={() => addEffect(effect.type)}>
                      <span class="mr-2">{effect.icon}</span>
                      {effect.name}
                    </button>
                  </li>
                )}
              </For>
            </ul>
          </div>
          <div class="dropdown dropdown-end">
            <button class="btn btn-xs btn-ghost" title="Preview Mode">
              <Eye class="w-3 h-3" />
            </button>
            <ul class="dropdown-content menu p-2 shadow bg-base-200 rounded-box w-32 text-xs">
              <li>
                <button 
                  onClick={() => setPreviewMode('split')}
                  class={previewMode() === 'split' ? 'active' : ''}
                >
                  Split View
                </button>
              </li>
              <li>
                <button 
                  onClick={() => setPreviewMode('before')}
                  class={previewMode() === 'before' ? 'active' : ''}
                >
                  Before
                </button>
              </li>
              <li>
                <button 
                  onClick={() => setPreviewMode('after')}
                  class={previewMode() === 'after' ? 'active' : ''}
                >
                  After
                </button>
              </li>
            </ul>
          </div>
          <button class="btn btn-xs btn-ghost" title="Settings">
            <Settings class="w-3 h-3" />
          </button>
        </div>
      </div>

      <div class="flex-1 flex">
        {/* Effects Stack */}
        <div class="w-56 border-r border-base-300 flex flex-col">
          <div class="p-2 border-b border-base-300 flex items-center justify-between">
            <div class="text-xs text-base-content/60 uppercase tracking-wide">Effects Stack</div>
            <div class="text-xs text-base-content/40">
              {effects().filter(e => e.enabled).length}/{effects().length}
            </div>
          </div>
          
          <div class="flex-1 overflow-y-auto">
            <div class="space-y-1 p-2">
              <For each={effects().sort((a, b) => a.order - b.order)}>
                {(effect) => (
                  <div
                    class={`p-2 cursor-pointer hover:bg-base-200 rounded border-l-2 group ${
                      effect.selected ? 'bg-base-200 border-primary' : 'border-transparent'
                    }`}
                    onClick={() => selectEffect(effect)}
                  >
                    <div class="flex items-center justify-between">
                      <div class="flex items-center space-x-2 flex-1 min-w-0">
                        <button
                          class={`w-3 h-3 rounded-full flex-shrink-0 ${
                            effect.enabled ? 'bg-success' : 'bg-base-300'
                          }`}
                          onClick={(e) => {
                            e.stopPropagation();
                            toggleEffect(effect.id);
                          }}
                        />
                        <span class="text-xs">{getEffectIcon(effect.type)}</span>
                        <span class="text-xs font-medium truncate">{effect.name}</span>
                      </div>
                      
                      <div class="flex items-center space-x-1 opacity-0 group-hover:opacity-100">
                        <button
                          class="btn btn-xs btn-ghost p-0 w-4 h-4"
                          onClick={(e) => {
                            e.stopPropagation();
                            moveEffect(effect.id, 'up');
                          }}
                          disabled={effect.order === 1}
                        >
                          <ArrowUp class="w-2 h-2" />
                        </button>
                        <button
                          class="btn btn-xs btn-ghost p-0 w-4 h-4"
                          onClick={(e) => {
                            e.stopPropagation();
                            moveEffect(effect.id, 'down');
                          }}
                          disabled={effect.order === effects().length}
                        >
                          <ArrowDown class="w-2 h-2" />
                        </button>
                      </div>
                    </div>
                    
                    <div class="flex items-center justify-between mt-1">
                      <div class="flex items-center space-x-2">
                        <span class="text-[10px] text-base-content/40">#{effect.order}</span>
                        <div class="w-12 h-1 bg-base-300 rounded-full overflow-hidden">
                          <div 
                            class="h-full bg-primary"
                            style={{ width: `${effect.intensity * 100}%` }}
                          />
                        </div>
                      </div>
                      <div class="flex items-center space-x-1">
                        <button
                          class="btn btn-xs btn-ghost p-0 w-4 h-4"
                          onClick={(e) => {
                            e.stopPropagation();
                            duplicateEffect(effect);
                          }}
                        >
                          <Copy class="w-2 h-2" />
                        </button>
                        <button
                          class="btn btn-xs btn-ghost p-0 w-4 h-4 text-error"
                          onClick={(e) => {
                            e.stopPropagation();
                            deleteEffect(effect.id);
                          }}
                        >
                          <Trash class="w-2 h-2" />
                        </button>
                      </div>
                    </div>
                    
                    <div class="text-[10px] text-base-content/40 mt-1 capitalize">
                      {effect.type.replace(/([A-Z])/g, ' $1').trim()}
                    </div>
                  </div>
                )}
              </For>
            </div>

            {effects().length === 0 && (
              <div class="flex-1 flex items-center justify-center p-4">
                <div class="text-center text-base-content/40">
                  <div class="w-8 h-8 bg-gradient-to-r from-pink-400 to-purple-500 rounded-full mx-auto mb-2"></div>
                  <p class="text-xs">No effects added</p>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Properties Panel */}
        <div class="flex-1 flex flex-col">
          {selectedEffect() ? (
            <>
              {/* Effect Info */}
              <div class="p-3 border-b border-base-300">
                <div class="flex items-center justify-between mb-2">
                  <h3 class="text-sm font-medium flex items-center">
                    <span class="mr-2">{getEffectIcon(selectedEffect().type)}</span>
                    {selectedEffect().name}
                  </h3>
                  <div class="flex items-center space-x-1">
                    <button
                      class={`btn btn-xs ${selectedEffect().enabled ? 'btn-success' : 'btn-ghost'}`}
                      onClick={() => toggleEffect(selectedEffect().id)}
                    >
                      {selectedEffect().enabled ? <Eye class="w-3 h-3" /> : <EyeOff class="w-3 h-3" />}
                    </button>
                    <button
                      class="btn btn-xs btn-ghost"
                      title="Reset to defaults"
                    >
                      <Reset class="w-3 h-3" />
                    </button>
                  </div>
                </div>
                <p class="text-xs text-base-content/60 capitalize">
                  {selectedEffect().type.replace(/([A-Z])/g, ' $1').trim()} effect
                </p>
              </div>

              {/* Properties */}
              <div class="flex-1 overflow-y-auto p-3 space-y-4">
                {/* Intensity */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Intensity</h4>
                  <div class="space-y-2">
                    <div class="flex items-center justify-between">
                      <span class="text-xs text-base-content/60">Amount</span>
                      <input
                        type="number"
                        step="0.01"
                        min="0"
                        max="2"
                        class="input input-xs input-bordered w-16 text-xs"
                        value={selectedEffect().intensity}
                      />
                    </div>
                    <input
                      type="range"
                      min="0"
                      max="2"
                      step="0.01"
                      value={selectedEffect().intensity}
                      class="range range-xs range-primary"
                    />
                  </div>
                </div>

                {/* Effect-specific settings */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Settings</h4>
                  {renderEffectSettings(selectedEffect())}
                </div>

                {/* Preview Options */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Preview</h4>
                  <div class="space-y-1">
                    <label class="flex items-center space-x-2 cursor-pointer">
                      <input 
                        type="radio" 
                        name="preview" 
                        class="radio radio-xs"
                        checked={previewMode() === 'split'}
                        onChange={() => setPreviewMode('split')}
                      />
                      <span class="text-xs text-base-content/60">Split View</span>
                    </label>
                    <label class="flex items-center space-x-2 cursor-pointer">
                      <input 
                        type="radio" 
                        name="preview" 
                        class="radio radio-xs"
                        checked={previewMode() === 'before'}
                        onChange={() => setPreviewMode('before')}
                      />
                      <span class="text-xs text-base-content/60">Before Only</span>
                    </label>
                    <label class="flex items-center space-x-2 cursor-pointer">
                      <input 
                        type="radio" 
                        name="preview" 
                        class="radio radio-xs"
                        checked={previewMode() === 'after'}
                        onChange={() => setPreviewMode('after')}
                      />
                      <span class="text-xs text-base-content/60">After Only</span>
                    </label>
                  </div>
                </div>

                {/* Order Control */}
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Order</h4>
                  <div class="flex items-center space-x-2">
                    <span class="text-xs text-base-content/60">Position:</span>
                    <span class="text-xs font-mono">#{selectedEffect().order}</span>
                    <div class="flex space-x-1 ml-auto">
                      <button
                        class="btn btn-xs btn-ghost"
                        onClick={() => moveEffect(selectedEffect().id, 'up')}
                        disabled={selectedEffect().order === 1}
                        title="Move up"
                      >
                        <ArrowUp class="w-3 h-3" />
                      </button>
                      <button
                        class="btn btn-xs btn-ghost"
                        onClick={() => moveEffect(selectedEffect().id, 'down')}
                        disabled={selectedEffect().order === effects().length}
                        title="Move down"
                      >
                        <ArrowDown class="w-3 h-3" />
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </>
          ) : (
            <div class="flex-1 flex items-center justify-center">
              <div class="text-center text-base-content/40">
                <div class="w-8 h-8 bg-gradient-to-r from-pink-400 to-purple-500 rounded-full mx-auto mb-2"></div>
                <p class="text-xs mb-2">No effect selected</p>
                <p class="text-xs">Add effects to enhance your renders</p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default PostProcessing;