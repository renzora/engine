import { createSignal, createEffect, Show } from 'solid-js';
import { renderStore, renderActions } from '@/render/store';
import { IconPalette, IconEye, IconDroplet, IconSun } from '@tabler/icons-solidjs';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { Color3 } from '@babylonjs/core/Maths/math.color';

function MaterialPanel(props) {
  const [materialSettings, setMaterialSettings] = createSignal({
    diffuseColor: '#ffffff',
    emissiveColor: '#000000',
    specularColor: '#ffffff',
    alpha: 1.0,
    roughness: 0.5,
    metallic: 0.0
  });
  
  // Section collapse state
  const [sectionsOpen, setSectionsOpen] = createSignal({
    colors: true,
    surface: true,
    presets: false
  });
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };
  
  const selectedObject = () => props.selectedObject;
  
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
    const obj = selectedObject();
    if (obj && obj.material) {
      const material = obj.material;
      
      // Handle both PBRMaterial and StandardMaterial
      const mainColor = material.baseColor || material.diffuseColor;
      
      setMaterialSettings({
        diffuseColor: mainColor ? color3ToHex(mainColor) : '#ffffff',
        emissiveColor: material.emissiveColor ? color3ToHex(material.emissiveColor) : '#000000',
        specularColor: material.specularColor ? color3ToHex(material.specularColor) : '#ffffff',
        alpha: material.alpha || 1.0,
        roughness: material.roughnessFactor || material.roughness || 0.5,
        metallic: material.metallicFactor || material.metallic || 0.0
      });
    } else {
      // Reset for objects without materials
      setMaterialSettings({
        diffuseColor: '#ffffff',
        emissiveColor: '#000000',
        specularColor: '#ffffff',
        alpha: 1.0,
        roughness: 0.5,
        metallic: 0.0
      });
    }
  });
  
  // Ensure object has a material
  const ensureMaterial = (obj) => {
    if (!obj.material) {
      const scene = renderStore.scene;
      if (scene) {
        // Create StandardMaterial for better compatibility and lighting response
        const material = new StandardMaterial(`${obj.name}_material`, scene);
        material.diffuseColor = new Color3(0.8, 0.8, 0.8); // Light gray default
        material.specularColor = new Color3(0.2, 0.2, 0.2); // Low specular for matte look
        material.emissiveColor = new Color3(0, 0, 0); // No emission by default
        obj.material = material;
      }
    }
    return obj.material;
  };
  
  const handleDiffuseColorChange = (color) => {
    const obj = selectedObject();
    if (!obj) return;
    
    const material = ensureMaterial(obj);
    if (material) {
      // Handle both PBRMaterial and StandardMaterial
      if (material.baseColor !== undefined) {
        // PBRMaterial
        material.baseColor = hexToColor3(color);
      } else {
        // StandardMaterial
        material.diffuseColor = hexToColor3(color);
      }
      setMaterialSettings(prev => ({ ...prev, diffuseColor: color }));
    }
  };
  
  const handleEmissiveColorChange = (color) => {
    const obj = selectedObject();
    if (!obj) return;
    
    const material = ensureMaterial(obj);
    if (material) {
      const emissiveColor = hexToColor3(color);
      // Reduce emissive intensity to prevent washing out
      emissiveColor.scaleInPlace(0.3); // Scale down to 30% intensity
      material.emissiveColor = emissiveColor;
      setMaterialSettings(prev => ({ ...prev, emissiveColor: color }));
    }
  };
  
  const handleSpecularColorChange = (color) => {
    const obj = selectedObject();
    if (!obj) return;
    
    const material = ensureMaterial(obj);
    if (material) {
      material.specularColor = hexToColor3(color);
      setMaterialSettings(prev => ({ ...prev, specularColor: color }));
    }
  };
  
  const handleAlphaChange = (alpha) => {
    const obj = selectedObject();
    if (!obj) return;
    
    const material = ensureMaterial(obj);
    if (material) {
      material.alpha = alpha;
      setMaterialSettings(prev => ({ ...prev, alpha }));
    }
  };
  
  const handleRoughnessChange = (roughness) => {
    const obj = selectedObject();
    if (!obj) return;
    
    const material = ensureMaterial(obj);
    if (material) {
      // Handle both PBRMaterial and StandardMaterial
      if (material.roughnessFactor !== undefined) {
        // PBRMaterial
        material.roughnessFactor = roughness;
      } else {
        // StandardMaterial - use specularPower (inverse relationship)
        material.specularPower = Math.max(1, (1 - roughness) * 128);
      }
      setMaterialSettings(prev => ({ ...prev, roughness }));
    }
  };
  
  const createNewMaterial = () => {
    const obj = selectedObject();
    if (!obj) return;
    
    const scene = renderStore.scene;
    if (scene) {
      const material = new StandardMaterial(`${obj.name}_material_${Date.now()}`, scene);
      material.diffuseColor = hexToColor3(materialSettings().diffuseColor);
      
      // Handle emissive with reduced intensity
      const emissiveColor = hexToColor3(materialSettings().emissiveColor);
      emissiveColor.scaleInPlace(0.3);
      material.emissiveColor = emissiveColor;
      
      material.specularColor = hexToColor3(materialSettings().specularColor);
      material.alpha = materialSettings().alpha;
      obj.material = material;
    }
  };
  
  return (
    <div class="h-full flex flex-col">
      <div class="flex-1 p-2 space-y-2">
        <Show 
          when={selectedObject()}
          fallback={
            <div class="flex flex-col items-center justify-center h-full text-base-content/60 text-center">
              <IconPalette class="w-8 h-8 mb-2 opacity-40" />
              <p class="text-sm">Select an object to edit material properties</p>
            </div>
          }
        >
          {/* Material Info */}
          <div class="bg-base-100 border-base-300 border rounded-lg p-3">
            <div class="flex items-center justify-between">
              <h3 class="text-sm font-medium text-base-content">Material</h3>
              <Show 
                when={selectedObject()?.material}
                fallback={
                  <button 
                    class="btn btn-primary btn-xs"
                    onClick={createNewMaterial}
                  >
                    Create Material
                  </button>
                }
              >
                <span class="text-xs text-base-content/60">
                  {selectedObject()?.material?.name || 'Unnamed Material'}
                </span>
              </Show>
            </div>
          </div>
          
          <Show when={selectedObject()?.material || true}>
            {/* Color Properties */}
            <div class="bg-base-100 border-base-300 border rounded-lg">
              <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().colors ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
                <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('colors')}>
                  <IconPalette class="w-3 h-3" />
                  Color Properties
                </div>
                <input
                  type="checkbox"
                  checked={sectionsOpen().colors}
                  onChange={(e) => {
                    e.stopPropagation();
                    toggleSection('colors');
                  }}
                  onClick={(e) => e.stopPropagation()}
                  class="toggle toggle-primary toggle-xs"
                />
              </div>
              <Show when={sectionsOpen().colors}>
                <div class="!p-2">
                  <div class="space-y-0.5">
                    {/* Main Color (Diffuse) */}
                    <div class="form-control">
                      <label class="text-xs font-medium text-base-content mb-1">Main Color</label>
                      <div class="flex items-center space-x-2">
                        <input 
                          type="color" 
                          class="w-10 h-8 rounded border border-base-300 cursor-pointer"
                          value={materialSettings().diffuseColor}
                          onChange={(e) => handleDiffuseColorChange(e.target.value)}
                        />
                        <input 
                          type="text" 
                          class="input input-xs input-bordered flex-1 font-mono"
                          value={materialSettings().diffuseColor}
                          onChange={(e) => handleDiffuseColorChange(e.target.value)}
                          placeholder="#ffffff"
                        />
                      </div>
                    </div>
                    
                    {/* Emissive Color */}
                    <div class="form-control">
                      <label class="text-xs font-medium text-base-content mb-1 flex items-center space-x-1">
                        <IconSun class="w-3 h-3" />
                        <span>Emissive Color</span>
                      </label>
                      <div class="flex items-center space-x-2">
                        <input 
                          type="color" 
                          class="w-10 h-8 rounded border border-base-300 cursor-pointer"
                          value={materialSettings().emissiveColor}
                          onChange={(e) => handleEmissiveColorChange(e.target.value)}
                        />
                        <input 
                          type="text" 
                          class="input input-xs input-bordered flex-1 font-mono"
                          value={materialSettings().emissiveColor}
                          onChange={(e) => handleEmissiveColorChange(e.target.value)}
                          placeholder="#000000"
                        />
                      </div>
                      <p class="text-xs text-base-content/50 mt-1">Glow effect - intensity automatically reduced</p>
                    </div>
                    
                    {/* Specular Color */}
                    <div class="form-control">
                      <label class="text-xs font-medium text-base-content mb-1 flex items-center space-x-1">
                        <IconDroplet class="w-3 h-3" />
                        <span>Specular Color</span>
                      </label>
                      <div class="flex items-center space-x-2">
                        <input 
                          type="color" 
                          class="w-10 h-8 rounded border border-base-300 cursor-pointer"
                          value={materialSettings().specularColor}
                          onChange={(e) => handleSpecularColorChange(e.target.value)}
                        />
                        <input 
                          type="text" 
                          class="input input-xs input-bordered flex-1 font-mono"
                          value={materialSettings().specularColor}
                          onChange={(e) => handleSpecularColorChange(e.target.value)}
                          placeholder="#ffffff"
                        />
                      </div>
                      <p class="text-xs text-base-content/50 mt-1">Color of highlights and reflections</p>
                    </div>
                  </div>
                </div>
              </Show>
            </div>
            
            {/* Surface Properties */}
            <div class="bg-base-100 border-base-300 border rounded-lg">
              <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().surface ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
                <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('surface')}>
                  <IconEye class="w-3 h-3" />
                  Surface Properties
                </div>
                <input
                  type="checkbox"
                  checked={sectionsOpen().surface}
                  onChange={(e) => {
                    e.stopPropagation();
                    toggleSection('surface');
                  }}
                  onClick={(e) => e.stopPropagation()}
                  class="toggle toggle-primary toggle-xs"
                />
              </div>
              <Show when={sectionsOpen().surface}>
                <div class="!p-2">
                  <div class="space-y-0.5">
                    {/* Alpha/Transparency */}
                    <div class="form-control">
                      <div class="flex items-center justify-between mb-1">
                        <label class="text-xs font-medium text-base-content">Opacity</label>
                        <span class="text-xs text-base-content/60">{Math.round(materialSettings().alpha * 100)}%</span>
                      </div>
                      <input 
                        type="range" 
                        class="range range-primary range-xs"
                        min="0" 
                        max="1" 
                        step="0.01"
                        value={materialSettings().alpha}
                        onChange={(e) => handleAlphaChange(parseFloat(e.target.value))}
                      />
                    </div>
                    
                    {/* Roughness */}
                    <div class="form-control">
                      <div class="flex items-center justify-between mb-1">
                        <label class="text-xs font-medium text-base-content">Roughness</label>
                        <span class="text-xs text-base-content/60">{Math.round(materialSettings().roughness * 100)}%</span>
                      </div>
                      <input 
                        type="range" 
                        class="range range-primary range-xs"
                        min="0" 
                        max="1" 
                        step="0.01"
                        value={materialSettings().roughness}
                        onChange={(e) => handleRoughnessChange(parseFloat(e.target.value))}
                      />
                      <p class="text-xs text-base-content/50 mt-1">Controls surface smoothness and reflections</p>
                    </div>
                  </div>
                </div>
              </Show>
            </div>
            
            {/* Quick Color Presets */}
            <div class="bg-base-100 border-base-300 border rounded-lg">
              <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().presets ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
                <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('presets')}>
                  <IconPalette class="w-3 h-3" />
                  Quick Colors
                </div>
                <input
                  type="checkbox"
                  checked={sectionsOpen().presets}
                  onChange={(e) => {
                    e.stopPropagation();
                    toggleSection('presets');
                  }}
                  onClick={(e) => e.stopPropagation()}
                  class="toggle toggle-primary toggle-xs"
                />
              </div>
              <Show when={sectionsOpen().presets}>
                <div class="!p-2">
                  <div class="grid grid-cols-6 gap-2">
                    {['#ff0000', '#00ff00', '#0000ff', '#ffff00', '#ff00ff', '#00ffff', 
                      '#ffffff', '#808080', '#000000', '#ffa500', '#800080', '#008000'].map(color => (
                      <button
                        class="w-8 h-8 rounded border-2 border-base-300 hover:border-primary transition-colors cursor-pointer"
                        style={{ 'background-color': color }}
                        onClick={() => handleDiffuseColorChange(color)}
                        title={color}
                      />
                    ))}
                  </div>
                </div>
              </Show>
            </div>
          </Show>
        </Show>
      </div>
    </div>
  );
}

export default MaterialPanel;