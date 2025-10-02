import { createSignal, createEffect, Show } from 'solid-js';
import { renderStore } from '@/render/store';
import { IconBulb, IconSun, IconTriangle, IconCircle, IconRectangle, IconEye, IconSettings, IconPalette } from '@tabler/icons-solidjs';
import { Color3 } from '@babylonjs/core/Maths/math.color';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';

function LightPanel(props) {
  const [lightSettings, setLightSettings] = createSignal({
    intensity: 1.0,
    diffuseColor: '#fff2cc',
    specularColor: '#ffffff',
    groundColor: '#4d4d4d', // For hemispheric lights
    enabled: true,
    // Spot light specific
    angle: 60, // degrees
    exponent: 2,
    // Area light specific
    width: 2,
    height: 2,
    // Directional light direction
    directionX: -1,
    directionY: -1,
    directionZ: -1
  });
  
  const selectedObject = () => props.selectedObject;
  
  // Get the actual light object from the container
  const getLight = () => {
    const obj = selectedObject();
    if (!obj || !obj.metadata?.isLightContainer) return null;
    
    // Find the actual light child
    const light = obj.getChildren().find(child => 
      child.getClassName && 
      ['DirectionalLight', 'PointLight', 'SpotLight', 'HemisphericLight', 'RectAreaLight'].includes(child.getClassName())
    );
    
    return light;
  };
  
  // Get light type
  const getLightType = () => {
    const obj = selectedObject();
    if (!obj || !obj.metadata?.isLightContainer) return null;
    return obj.metadata.lightType;
  };
  
  // Convert Color3 to hex string
  const color3ToHex = (color3) => {
    if (!color3) return '#ffffff';
    const r = Math.round(color3.r * 255).toString(16).padStart(2, '0');
    const g = Math.round(color3.g * 255).toString(16).padStart(2, '0');
    const b = Math.round(color3.b * 255).toString(16).padStart(2, '0');
    return `#${r}${g}${b}`;
  };
  
  // Convert hex string to Color3
  const hexToColor3 = (hex) => {
    const r = parseInt(hex.slice(1, 3), 16) / 255;
    const g = parseInt(hex.slice(3, 5), 16) / 255;
    const b = parseInt(hex.slice(5, 7), 16) / 255;
    return new Color3(r, g, b);
  };
  
  // Update settings when selected object changes
  createEffect(() => {
    const light = getLight();
    const lightType = getLightType();
    
    if (light) {
      const settings = {
        intensity: light.intensity || 1.0,
        diffuseColor: light.diffuse ? color3ToHex(light.diffuse) : '#fff2cc',
        specularColor: light.specular ? color3ToHex(light.specular) : '#ffffff',
        enabled: light.isEnabled()
      };
      
      // Add type-specific properties
      if (lightType === 'hemispheric' && light.groundColor) {
        settings.groundColor = color3ToHex(light.groundColor);
      }
      
      if (lightType === 'spot') {
        settings.angle = light.angle ? (light.angle * 180 / Math.PI) : 60; // Convert to degrees
        settings.exponent = light.exponent || 2;
      }
      
      if (lightType === 'rectArea') {
        settings.width = light.width || 2;
        settings.height = light.height || 2;
      }
      
      if (lightType === 'directional' && light.direction) {
        settings.directionX = light.direction.x;
        settings.directionY = light.direction.y;
        settings.directionZ = light.direction.z;
      }
      
      setLightSettings(settings);
    }
  });
  
  // Apply changes to the light
  const applyLightSettings = (property, value) => {
    const light = getLight();
    const lightType = getLightType();
    
    if (!light) return;
    
    switch (property) {
      case 'intensity':
        light.intensity = parseFloat(value);
        break;
      case 'diffuseColor':
        light.diffuse = hexToColor3(value);
        break;
      case 'specularColor':
        light.specular = hexToColor3(value);
        break;
      case 'groundColor':
        if (lightType === 'hemispheric') {
          light.groundColor = hexToColor3(value);
        }
        break;
      case 'enabled':
        light.setEnabled(value);
        break;
      case 'angle':
        if (lightType === 'spot') {
          light.angle = parseFloat(value) * Math.PI / 180; // Convert to radians
        }
        break;
      case 'exponent':
        if (lightType === 'spot') {
          light.exponent = parseFloat(value);
        }
        break;
      case 'width':
        if (lightType === 'rectArea') {
          light.width = parseFloat(value);
        }
        break;
      case 'height':
        if (lightType === 'rectArea') {
          light.height = parseFloat(value);
        }
        break;
      case 'direction':
        if (lightType === 'directional') {
          light.direction = new Vector3(value.x, value.y, value.z);
        }
        break;
    }
    
    // Update the settings state
    setLightSettings(prev => ({ ...prev, [property]: value }));
  };
  
  // Get light type icon
  const getLightIcon = () => {
    const lightType = getLightType();
    switch (lightType) {
      case 'directional': return IconSun;
      case 'point': return IconBulb;
      case 'spot': return IconTriangle;
      case 'hemispheric': return IconCircle;
      case 'rectArea': return IconRectangle;
      default: return IconBulb;
    }
  };
  
  // Get light type display name
  const getLightTypeName = () => {
    const lightType = getLightType();
    switch (lightType) {
      case 'directional': return 'Directional Light';
      case 'point': return 'Point Light';
      case 'spot': return 'Spot Light';
      case 'hemispheric': return 'Hemispheric Light';
      case 'rectArea': return 'Rectangular Area Light';
      default: return 'Unknown Light';
    }
  };
  
  // Check if selected object is a light
  const isLightSelected = () => {
    const obj = selectedObject();
    return obj && obj.metadata?.isLightContainer;
  };
  
  return (
    <div class="h-full overflow-y-auto p-4 space-y-6 bg-base-100">
      <Show 
        when={isLightSelected()}
        fallback={
          <div class="flex flex-col items-center justify-center h-full text-base-content/60 text-center">
            <IconBulb class="w-8 h-8 mb-2 opacity-40" />
            <p class="text-sm">Select a light to edit its properties</p>
          </div>
        }
      >
        {/* Light Info */}
        <div class="space-y-2">
          <div class="flex items-center justify-between">
            <h3 class="text-sm font-medium text-base-content">Light</h3>
            <div class="flex items-center space-x-2">
              {(() => {
                const IconComponent = getLightIcon();
                return <IconComponent class="w-4 h-4 text-primary" />;
              })()}
              <span class="text-xs text-base-content/60">
                {getLightTypeName()}
              </span>
            </div>
          </div>
          <div class="text-xs text-base-content/50">{selectedObject()?.name}</div>
        </div>
        
        {/* Basic Properties */}
        <div class="space-y-4">
          <div class="flex items-center space-x-2 pb-2 border-b border-base-300">
            <IconSettings class="w-4 h-4 text-primary" />
            <h3 class="text-sm font-medium text-base-content">Basic Properties</h3>
          </div>
          
          {/* Enabled Toggle */}
          <div class="flex items-center justify-between">
            <label class="text-xs font-medium text-base-content flex items-center space-x-1">
              <IconEye class="w-3 h-3" />
              <span>Enabled</span>
            </label>
            <input
              type="checkbox"
              checked={lightSettings().enabled}
              onChange={(e) => applyLightSettings('enabled', e.target.checked)}
              class="toggle toggle-primary toggle-sm"
            />
          </div>
          
          {/* Intensity */}
          <div class="space-y-2">
            <div class="flex items-center justify-between">
              <label class="text-xs font-medium text-base-content">Intensity</label>
              <span class="text-xs text-base-content/60">{Number(lightSettings().intensity || 0).toFixed(2)}</span>
            </div>
            <input
              type="range"
              min="0"
              max="20"
              step="0.1"
              value={lightSettings().intensity}
              onInput={(e) => applyLightSettings('intensity', e.target.value)}
              class="range range-primary range-sm"
            />
          </div>
        </div>
        
        {/* Color Properties */}
        <div class="space-y-4">
          <div class="flex items-center space-x-2 pb-2 border-b border-base-300">
            <IconPalette class="w-4 h-4 text-primary" />
            <h3 class="text-sm font-medium text-base-content">Color Properties</h3>
          </div>
          
          {/* Diffuse Color */}
          <div class="space-y-2">
            <label class="text-xs font-medium text-base-content">Diffuse Color</label>
            <div class="flex items-center space-x-3">
              <input 
                type="color" 
                class="w-12 h-8 rounded border border-base-300 cursor-pointer"
                value={lightSettings().diffuseColor}
                onInput={(e) => applyLightSettings('diffuseColor', e.target.value)}
              />
              <input 
                type="text" 
                class="input input-sm input-bordered flex-1 font-mono text-xs"
                value={lightSettings().diffuseColor}
                onChange={(e) => applyLightSettings('diffuseColor', e.target.value)}
                placeholder="#fff2cc"
              />
            </div>
          </div>
          
          {/* Specular Color */}
          <div class="space-y-2">
            <label class="text-xs font-medium text-base-content">Specular Color</label>
            <div class="flex items-center space-x-3">
              <input 
                type="color" 
                class="w-12 h-8 rounded border border-base-300 cursor-pointer"
                value={lightSettings().specularColor}
                onInput={(e) => applyLightSettings('specularColor', e.target.value)}
              />
              <input 
                type="text" 
                class="input input-sm input-bordered flex-1 font-mono text-xs"
                value={lightSettings().specularColor}
                onChange={(e) => applyLightSettings('specularColor', e.target.value)}
                placeholder="#ffffff"
              />
            </div>
          </div>
        </div>
        
        {/* Type-specific Properties */}
        <Show when={getLightType() === 'hemispheric'}>
          <div class="space-y-4">
            <div class="flex items-center space-x-2 pb-2 border-b border-base-300">
              <IconCircle class="w-4 h-4 text-primary" />
              <h3 class="text-sm font-medium text-base-content">Hemispheric Properties</h3>
            </div>
            
            <div class="space-y-2">
              <label class="text-xs font-medium text-base-content">Ground Color</label>
              <div class="flex items-center space-x-3">
                <input 
                  type="color" 
                  class="w-12 h-8 rounded border border-base-300 cursor-pointer"
                  value={lightSettings().groundColor}
                  onInput={(e) => applyLightSettings('groundColor', e.target.value)}
                />
                <input 
                  type="text" 
                  class="input input-sm input-bordered flex-1 font-mono text-xs"
                  value={lightSettings().groundColor}
                  onChange={(e) => applyLightSettings('groundColor', e.target.value)}
                  placeholder="#4d4d4d"
                />
              </div>
            </div>
          </div>
        </Show>
        
        <Show when={getLightType() === 'spot'}>
          <div class="space-y-4">
            <div class="flex items-center space-x-2 pb-2 border-b border-base-300">
              <IconTriangle class="w-4 h-4 text-primary" />
              <h3 class="text-sm font-medium text-base-content">Spot Light Properties</h3>
            </div>
            
            <div class="space-y-2">
              <div class="flex items-center justify-between">
                <label class="text-xs font-medium text-base-content">Angle (degrees)</label>
                <span class="text-xs text-base-content/60">{Number(lightSettings().angle || 0).toFixed(0)}°</span>
              </div>
              <input
                type="range"
                min="1"
                max="180"
                step="1"
                value={lightSettings().angle}
                onInput={(e) => applyLightSettings('angle', e.target.value)}
                class="range range-primary range-sm"
              />
            </div>
            
            <div class="space-y-2">
              <div class="flex items-center justify-between">
                <label class="text-xs font-medium text-base-content">Exponent</label>
                <span class="text-xs text-base-content/60">{Number(lightSettings().exponent || 0).toFixed(1)}</span>
              </div>
              <input
                type="range"
                min="0.1"
                max="10"
                step="0.1"
                value={lightSettings().exponent}
                onInput={(e) => applyLightSettings('exponent', e.target.value)}
                class="range range-primary range-sm"
              />
            </div>
          </div>
        </Show>
        
        <Show when={getLightType() === 'rectArea'}>
          <div class="space-y-4">
            <div class="flex items-center space-x-2 pb-2 border-b border-base-300">
              <IconRectangle class="w-4 h-4 text-primary" />
              <h3 class="text-sm font-medium text-base-content">Area Light Properties</h3>
            </div>
            
            <div class="space-y-2">
              <div class="flex items-center justify-between">
                <label class="text-xs font-medium text-base-content">Width</label>
                <span class="text-xs text-base-content/60">{Number(lightSettings().width || 0).toFixed(1)}</span>
              </div>
              <input
                type="range"
                min="0.1"
                max="20"
                step="0.1"
                value={lightSettings().width}
                onInput={(e) => applyLightSettings('width', e.target.value)}
                class="range range-primary range-sm"
              />
            </div>
            
            <div class="space-y-2">
              <div class="flex items-center justify-between">
                <label class="text-xs font-medium text-base-content">Height</label>
                <span class="text-xs text-base-content/60">{Number(lightSettings().height || 0).toFixed(1)}</span>
              </div>
              <input
                type="range"
                min="0.1"
                max="20"
                step="0.1"
                value={lightSettings().height}
                onInput={(e) => applyLightSettings('height', e.target.value)}
                class="range range-primary range-sm"
              />
            </div>
          </div>
        </Show>
        
        <Show when={getLightType() === 'directional'}>
          <div class="space-y-4">
            <div class="flex items-center space-x-2 pb-2 border-b border-base-300">
              <IconSun class="w-4 h-4 text-primary" />
              <h3 class="text-sm font-medium text-base-content">Directional Properties</h3>
            </div>
            
            <div class="space-y-3">
              <div class="space-y-2">
                <div class="flex items-center justify-between">
                  <label class="text-xs font-medium text-base-content">Direction X</label>
                  <span class="text-xs text-base-content/60">{Number(lightSettings().directionX || 0).toFixed(1)}</span>
                </div>
                <input
                  type="range"
                  min="-1"
                  max="1"
                  step="0.1"
                  value={lightSettings().directionX}
                  onInput={(e) => {
                    const newDirection = {
                      x: parseFloat(e.target.value),
                      y: lightSettings().directionY,
                      z: lightSettings().directionZ
                    };
                    applyLightSettings('direction', newDirection);
                  }}
                  class="range range-primary range-sm"
                />
              </div>
              
              <div class="space-y-2">
                <div class="flex items-center justify-between">
                  <label class="text-xs font-medium text-base-content">Direction Y</label>
                  <span class="text-xs text-base-content/60">{Number(lightSettings().directionY || 0).toFixed(1)}</span>
                </div>
                <input
                  type="range"
                  min="-1"
                  max="1"
                  step="0.1"
                  value={lightSettings().directionY}
                  onInput={(e) => {
                    const newDirection = {
                      x: lightSettings().directionX,
                      y: parseFloat(e.target.value),
                      z: lightSettings().directionZ
                    };
                    applyLightSettings('direction', newDirection);
                  }}
                  class="range range-primary range-sm"
                />
              </div>
              
              <div class="space-y-2">
                <div class="flex items-center justify-between">
                  <label class="text-xs font-medium text-base-content">Direction Z</label>
                  <span class="text-xs text-base-content/60">{Number(lightSettings().directionZ || 0).toFixed(1)}</span>
                </div>
                <input
                  type="range"
                  min="-1"
                  max="1"
                  step="0.1"
                  value={lightSettings().directionZ}
                  onInput={(e) => {
                    const newDirection = {
                      x: lightSettings().directionX,
                      y: lightSettings().directionY,
                      z: parseFloat(e.target.value)
                    };
                    applyLightSettings('direction', newDirection);
                  }}
                  class="range range-primary range-sm"
                />
              </div>
            </div>
          </div>
        </Show>
      </Show>
    </div>
  );
}

export default LightPanel;