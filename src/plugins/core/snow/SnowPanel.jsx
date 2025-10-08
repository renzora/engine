import { createEffect, createMemo, createSignal } from 'solid-js';
import { renderStore, renderActions } from '@/render/store';
import { IconSnowflake, IconSettings, IconRefresh, IconEye } from '@tabler/icons-solidjs';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { Color4 } from '@babylonjs/core/Maths/math.color';
import { ParticleSystem } from '@babylonjs/core/Particles/particleSystem';
import { DynamicTexture } from '@babylonjs/core/Materials/Textures/dynamicTexture';

export default function SnowPanel() {
  // Get lighting settings from render store
  const lighting = () => renderStore.lighting;
  
  // Generic setter function
  const setSetting = (key, value) => renderActions.setLightingSetting(key, value);
  
  // Section collapse state
  const [sectionsOpen, setSectionsOpen] = createSignal({
    snow: true,
    intensity: true,
    appearance: true,
    presets: true
  });
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };

  let snowParticleSystem = null;

  // Cleanup existing snow system
  const _cleanupSnowSystem = () => {
    if (snowParticleSystem) {
      console.log('🗑️ Cleaning up snow particle system...');
      snowParticleSystem.stop();
      snowParticleSystem.dispose();
      snowParticleSystem = null;
    }
  };
  
  // Create snow particle system
  const createSnowSystem = (scene) => {
    if (!scene || snowParticleSystem) return;

    console.log('❄️ Creating snow particle system...');

    // Create particle system
    snowParticleSystem = new ParticleSystem("snow", 800, scene);
    
    // Use a simple dynamic texture instead of canvas
    const texture = new DynamicTexture("snowTexture", {width: 16, height: 16}, scene);
    const ctx = texture.getContext();
    
    // Draw snowflake directly on texture context
    ctx.fillStyle = 'white';
    ctx.beginPath();
    ctx.arc(8, 8, 6, 0, Math.PI * 2);
    ctx.fill();
    
    // Add sparkle effect
    ctx.fillStyle = 'rgba(255, 255, 255, 0.8)';
    ctx.beginPath();
    ctx.arc(6, 6, 2, 0, Math.PI * 2);
    ctx.fill();
    ctx.beginPath();
    ctx.arc(10, 10, 1.5, 0, Math.PI * 2);
    ctx.fill();
    
    texture.update();
    snowParticleSystem.particleTexture = texture;
    
    // Snow emitter setup
    snowParticleSystem.emitter = new Vector3(0, 50, 0);
    snowParticleSystem.minEmitBox = new Vector3(-50, 0, -50);
    snowParticleSystem.maxEmitBox = new Vector3(50, 0, 50);
    
    // Particle properties
    snowParticleSystem.color1 = new Color4(1.0, 1.0, 1.0, 0.9);
    snowParticleSystem.color2 = new Color4(0.9, 0.9, 1.0, 0.8);
    snowParticleSystem.colorDead = new Color4(0.8, 0.8, 0.9, 0.0);
    
    snowParticleSystem.minSize = 0.5;
    snowParticleSystem.maxSize = 2.0;
    snowParticleSystem.minLifeTime = 5.0;
    snowParticleSystem.maxLifeTime = 10.0;
    
    snowParticleSystem.emitRate = 100;
    
    // Snow direction (gentle falling)
    snowParticleSystem.direction1 = new Vector3(-1, -3, -1);
    snowParticleSystem.direction2 = new Vector3(1, -5, 1);
    
    snowParticleSystem.gravity = new Vector3(0, -2, 0);
    
    snowParticleSystem.minAngularSpeed = -0.5;
    snowParticleSystem.maxAngularSpeed = 0.5;
    
    snowParticleSystem.minInitialRotation = 0;
    snowParticleSystem.maxInitialRotation = Math.PI * 2;
    
    // Start the particle system
    if (lighting().snowEnabled) {
      snowParticleSystem.start();
      console.log('❄️ Snow particle system started');
    }
    
    console.log('❄️ Snow particle system created successfully');
    return snowParticleSystem;
  };
  
  // Apply snow changes to the scene in real-time
  createEffect(() => {
    const scene = window._cleanBabylonScene || renderStore.scene;
    if (!scene) {
      console.log('❄️ No scene available for snow');
      return;
    }
    
    console.log('❄️ Snow effect updating, scene available:', !!scene);
    const l = lighting();
    
    if (l.snowEnabled) {
      // Only create snow system if it doesn't exist or is disposed
      if (!snowParticleSystem || snowParticleSystem.isDisposed()) {
        createSnowSystem(scene);
      }
      
      if (snowParticleSystem && !snowParticleSystem.isDisposed()) {
        // Update snow properties
        snowParticleSystem.emitRate = (l.snowIntensity || 1.0) * 100;
        snowParticleSystem.minSize = (l.snowSize || 1.0) * 0.5;
        snowParticleSystem.maxSize = (l.snowSize || 1.0) * 2.0;
        
        const windX = (l.snowWind || 0) * 2;
        const windZ = (l.snowWind || 0) * 1.5;
        snowParticleSystem.direction1 = new Vector3(-1 + windX, -3, -1 + windZ);
        snowParticleSystem.direction2 = new Vector3(1 + windX, -5, 1 + windZ);
        
        // Snow falls slower than rain
        snowParticleSystem.gravity = new Vector3(windX * 0.5, -2 * (l.snowGravity || 1.0), windZ * 0.5);
        
        // Apply snow color (usually white/blue tinted)
        const snowColor = l.snowColor || [1.0, 1.0, 1.0];
        snowParticleSystem.color1 = new Color4(snowColor[0], snowColor[1], snowColor[2], 0.9);
        snowParticleSystem.color2 = new Color4(snowColor[0] * 0.9, snowColor[1] * 0.9, snowColor[2], 0.8);
        
        snowParticleSystem.start();
      }
    } else if (snowParticleSystem && !snowParticleSystem.isDisposed()) {
      snowParticleSystem.stop();
    }
  });

  // Note: We don't cleanup snow when switching tabs because it should persist as an environmental effect
  // Snow will be cleaned up when explicitly disabled or when the scene is disposed

  // Convert RGB array to hex color for color input
  const rgbToHex = (rgb) => {
    const r = Math.round(rgb[0] * 255).toString(16).padStart(2, '0');
    const g = Math.round(rgb[1] * 255).toString(16).padStart(2, '0');
    const b = Math.round(rgb[2] * 255).toString(16).padStart(2, '0');
    return `#${r}${g}${b}`;
  };

  // Convert hex color to RGB array
  const hexToRgb = (hex) => {
    const r = parseInt(hex.slice(1, 3), 16) / 255;
    const g = parseInt(hex.slice(3, 5), 16) / 255;
    const b = parseInt(hex.slice(5, 7), 16) / 255;
    return [r, g, b];
  };

  const SliderControl = ({ label, getValue, min, max, step, onChange, unit = '', disabled = false }) => {
    const displayValue = createMemo(() => {
      const value = getValue();
      if (typeof value !== 'number') return value;
      if (step < 0.01) return value.toFixed(4);
      if (step < 0.1) return value.toFixed(2);
      if (step < 1) return value.toFixed(1);
      return value.toFixed(0);
    });
    
    return (
      <div class={disabled ? 'opacity-50' : ''}>
        <label class="block text-xs text-base-content/80 mb-1">
          {label}: {displayValue()}{unit}
        </label>
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={getValue()}
          onInput={(e) => onChange(parseFloat(e.target.value))}
          class="range range-primary w-full range-xs"
          disabled={disabled}
        />
      </div>
    );
  };

  const ColorControl = ({ label, value, onChange, disabled = false }) => (
    <div class={disabled ? 'opacity-50' : ''}>
      <label class="block text-xs text-base-content/80 mb-1">{label}</label>
      <input
        type="color"
        value={rgbToHex(value)}
        onInput={(e) => onChange(hexToRgb(e.target.value))}
        class="w-full h-6 rounded border border-base-300"
        disabled={disabled}
      />
    </div>
  );

  const ToggleControl = ({ label, value, onChange, icon, description }) => (
    <div class="space-y-1">
      <div class="flex items-center justify-between">
        <label class="text-xs text-base-content/80 flex items-center gap-1">
          {icon && <icon class="w-3 h-3" />}
          {label}
        </label>
        <input
          type="checkbox"
          checked={value}
          onChange={(e) => onChange(e.target.checked)}
          class="toggle toggle-primary toggle-xs"
        />
      </div>
      {description && (
        <p class="text-xs text-base-content/60">{description}</p>
      )}
    </div>
  );

  const resetSnowSettings = () => {
    setSetting('snowEnabled', false);
    setSetting('snowIntensity', 1.0);
    setSetting('snowSize', 1.0);
    setSetting('snowWind', 0.0);
    setSetting('snowGravity', 1.0);
    setSetting('snowColor', [1.0, 1.0, 1.0]);
  };

  return (
    <div class="h-full flex flex-col">
      <div class="flex-1 p-2 space-y-2">
      
      {/* Snow Enable/Disable */}
      <div class="bg-base-100 border-base-300 border rounded-lg">
        <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().snow ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('snow')}>
          <IconEye class="w-3 h-3" />
          Snow Settings
        </div>
        {sectionsOpen().snow && (
          <div class="!p-2">
            <div class="space-y-2">
              <ToggleControl 
                label="Enable Snow" 
                value={lighting().snowEnabled || false} 
                onChange={(v) => setSetting('snowEnabled', v)}
                icon={IconSnowflake}
                description="Enable snow particle effects"
              />
            </div>
          </div>
        )}
      </div>

      {/* Intensity Controls */}
      {lighting().snowEnabled && (
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().intensity ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('intensity')}>
            <IconSnowflake class="w-3 h-3" />
            Intensity Controls
          </div>
          {sectionsOpen().intensity && (
            <div class="!p-2">
              <div class="space-y-2">
                <SliderControl 
                  label="Snow Intensity" 
                  getValue={() => lighting().snowIntensity || 1.0} 
                  min={0.1} 
                  max={3.0} 
                  step={0.1} 
                  onChange={(v) => setSetting('snowIntensity', v)}
                />
                
                <SliderControl 
                  label="Snowflake Size" 
                  getValue={() => lighting().snowSize || 1.0} 
                  min={0.3} 
                  max={3.0} 
                  step={0.1} 
                  onChange={(v) => setSetting('snowSize', v)}
                />
                
                <SliderControl 
                  label="Wind Effect" 
                  getValue={() => lighting().snowWind || 0.0} 
                  min={-2.0} 
                  max={2.0} 
                  step={0.1} 
                  onChange={(v) => setSetting('snowWind', v)}
                />
                
                <SliderControl 
                  label="Fall Speed" 
                  getValue={() => lighting().snowGravity || 1.0} 
                  min={0.3} 
                  max={2.0} 
                  step={0.1} 
                  onChange={(v) => setSetting('snowGravity', v)}
                />
                <p class="text-xs text-base-content/60">
                  Wind and fall speed affect how snowflakes move through the air
                </p>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Appearance */}
      {lighting().snowEnabled && (
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().appearance ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('appearance')}>
            <IconSettings class="w-3 h-3" />
            Appearance
          </div>
          {sectionsOpen().appearance && (
            <div class="!p-2">
              <div class="space-y-2">
                <ColorControl 
                  label="Snow Color" 
                  value={lighting().snowColor || [1.0, 1.0, 1.0]} 
                  onChange={(v) => setSetting('snowColor', v)} 
                />
              </div>
            </div>
          )}
        </div>
      )}

      {/* Presets */}
      {lighting().snowEnabled && (
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().presets ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('presets')}>
            <IconSnowflake class="w-3 h-3" />
            Snow Presets
          </div>
          {sectionsOpen().presets && (
            <div class="!p-2">
              <div class="grid grid-cols-2 gap-2">
                <button 
                  onClick={() => {
                    setSetting('snowIntensity', 0.5);
                    setSetting('snowSize', 0.8);
                    setSetting('snowWind', 0.0);
                    setSetting('snowGravity', 0.8);
                    setSetting('snowColor', [1.0, 1.0, 1.0]);
                  }}
                  class="btn btn-xs btn-outline"
                >
                  Light Snow
                </button>
                <button 
                  onClick={() => {
                    setSetting('snowIntensity', 2.0);
                    setSetting('snowSize', 1.5);
                    setSetting('snowWind', 0.8);
                    setSetting('snowGravity', 1.2);
                    setSetting('snowColor', [0.95, 0.95, 1.0]);
                  }}
                  class="btn btn-xs btn-outline"
                >
                  Heavy Snow
                </button>
                <button 
                  onClick={() => {
                    setSetting('snowIntensity', 2.5);
                    setSetting('snowSize', 1.8);
                    setSetting('snowWind', 1.5);
                    setSetting('snowGravity', 1.5);
                    setSetting('snowColor', [0.9, 0.9, 0.95]);
                  }}
                  class="btn btn-xs btn-outline"
                >
                  Blizzard
                </button>
                <button 
                  onClick={() => {
                    setSetting('snowIntensity', 0.3);
                    setSetting('snowSize', 2.0);
                    setSetting('snowWind', -0.2);
                    setSetting('snowGravity', 0.5);
                    setSetting('snowColor', [1.0, 1.0, 1.0]);
                  }}
                  class="btn btn-xs btn-outline"
                >
                  Gentle Flakes
                </button>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Reset Button */}
      <div class="pt-2 border-t border-base-300">
        <button 
          onClick={resetSnowSettings}
          class="btn btn-outline btn-error btn-xs w-full flex items-center gap-2"
        >
          <IconRefresh class="w-3 h-3" />
          Reset Snow Settings
        </button>
      </div>
      </div>
    </div>
  );
}