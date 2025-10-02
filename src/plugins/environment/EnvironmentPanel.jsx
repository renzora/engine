import { createSignal, createEffect } from 'solid-js';
import { renderStore } from '@/render/store';
import { IconSun } from '@tabler/icons-solidjs';
import { Color3, Color4 } from '@babylonjs/core/Maths/math.color';
import { Texture } from '@babylonjs/core/Materials/Textures/texture';
import { HDRCubeTexture } from '@babylonjs/core/Materials/Textures/hdrCubeTexture';
import { CubeTexture } from '@babylonjs/core/Materials/Textures/cubeTexture';
import { bridgeService } from '@/plugins/core/bridge';

function EnvironmentPanel(props) {
  // Skybox controls
  const [skyboxColor, setSkyboxColor] = createSignal('#87CEEB'); // Sky blue default
  const [skyboxVisible, setSkyboxVisible] = createSignal(true);
  const [skyboxBrightness, setSkyboxBrightness] = createSignal(1.0);
  const [skyboxImage, setSkyboxImage] = createSignal(null); // For uploaded image
  const [isDragging, setIsDragging] = createSignal(false);
  
  // Environment controls
  const [environmentIntensity, setEnvironmentIntensity] = createSignal(1.0);
  
  // Fog controls
  const [fogEnabled, setFogEnabled] = createSignal(false);
  const [fogColor, setFogColor] = createSignal('#CCCCCC');
  const [fogDensity, setFogDensity] = createSignal(0.01);
  const [fogStart, setFogStart] = createSignal(10);
  const [fogEnd, setFogEnd] = createSignal(100);

  // Get selected object
  const selectedObject = () => props.selectedObject || renderStore.selectedObject;

  // Handle image file upload
  const handleImageUpload = (file) => {
    if (!file || !file.type.startsWith('image/')) {
      console.error('Please upload a valid image file');
      return;
    }

    const reader = new FileReader();
    reader.onload = (e) => {
      const imageUrl = e.target.result;
      setSkyboxImage(imageUrl);
      console.log('📸 Skybox image uploaded:', file.name);
    };
    reader.readAsDataURL(file);
  };

  // Handle HDR/EXR file upload from asset library using native BabylonJS support
  const handleHDRUpload = async (assetData) => {
    try {
      console.log('🌍 Loading HDR/EXR file with native BabylonJS support:', assetData.path);
      
      // Use the full asset ID which includes the project path
      const fullUrl = `http://localhost:3001/file/${assetData.id}`;
      
      // Store HDR URL for native BabylonJS HDR loading
      setSkyboxImage({
        type: 'hdr',
        url: fullUrl,
        name: assetData.name
      });
      
      console.log('✅ HDR file set for native BabylonJS loading:', assetData.name);
      
    } catch (error) {
      console.error('❌ Error setting HDR file:', error);
    }
  };

  // Handle regular image asset upload from asset library
  const handleAssetImageUpload = async (assetData) => {
    try {
      console.log('🌍 Loading image asset:', assetData.path);
      
      // Use the full asset ID which includes the project path
      const fullUrl = `http://localhost:3001/file/${assetData.id}`;
      
      setSkyboxImage(fullUrl);
      console.log('✅ Image skybox set:', assetData.name, 'URL:', fullUrl);
      
    } catch (error) {
      console.error('❌ Error loading image asset:', error);
    }
  };

  // Handle drag and drop
  const handleDragOver = (e) => {
    e.preventDefault();
    
    // Check for asset drag or regular file drag
    if (e.dataTransfer.types.includes('application/x-asset-drag') || 
        e.dataTransfer.types.includes('Files')) {
      setIsDragging(true);
      e.dataTransfer.dropEffect = 'copy';
    }
  };

  const handleDragLeave = (e) => {
    e.preventDefault();
    setIsDragging(false);
  };

  const handleDrop = (e) => {
    e.preventDefault();
    setIsDragging(false);
    
    // Handle asset drag from asset library
    if (e.dataTransfer.types.includes('application/x-asset-drag')) {
      try {
        const assetData = JSON.parse(e.dataTransfer.getData('application/json'));
        console.log('🌍 Asset dropped on environment panel:', assetData);
        
        if (assetData.type === 'asset' && assetData.assetType === 'file') {
          const extension = assetData.extension?.toLowerCase();
          
          // Check if it's an HDR/EXR file - now directly supported!
          if (['.hdr', '.exr'].includes(extension)) {
            console.log('🌍 HDR/EXR file detected:', assetData.name);
            handleHDRUpload(assetData);
            return;
          } else if (['.jpg', '.jpeg', '.png', '.webp'].includes(extension)) {
            console.log('🌍 Standard image file detected:', assetData.name);
            handleAssetImageUpload(assetData);
            return;
          } else {
            console.warn('⚠️ Unsupported file type for skybox:', extension);
            return;
          }
        }
      } catch (error) {
        console.error('❌ Error parsing asset drag data:', error);
      }
    }
    
    // Handle regular file drag
    const files = e.dataTransfer.files;
    if (files.length > 0) {
      handleImageUpload(files[0]);
    }
  };

  // Handle file input change
  const handleFileChange = (e) => {
    const files = e.target.files;
    if (files.length > 0) {
      const file = files[0];
      const fileName = file.name.toLowerCase();
      
      // Check if it's HDR/EXR file
      if (fileName.endsWith('.hdr') || fileName.endsWith('.exr')) {
        console.log('🌍 HDR/EXR file selected from file input:', file.name);
        
        // Create file URL for HDR/EXR files
        const fileUrl = URL.createObjectURL(file);
        setSkyboxImage({
          type: 'hdr',
          url: fileUrl,
          name: file.name,
          isFileInput: true // Track that this is from file input for cleanup
        });
        
        console.log('✅ HDR/EXR file loaded from file input:', file.name);
      } else {
        // Handle regular image files
        handleImageUpload(file);
      }
    }
  };

  // Clear skybox image
  const clearSkyboxImage = () => {
    const currentImage = skyboxImage();
    
    // Clean up blob URL if it was created from file input
    if (currentImage && typeof currentImage === 'object' && currentImage.isFileInput && currentImage.url) {
      URL.revokeObjectURL(currentImage.url);
      console.log('🗑️ Cleaned up blob URL for file input');
    }
    
    setSkyboxImage(null);
    console.log('🗑️ Skybox image cleared');
  };

  // Initialize settings from scene and selected environment object
  createEffect(() => {
    const scene = renderStore.scene;
    const obj = selectedObject();
    
    if (!scene) return;
    
    // Only proceed if we have an environment object selected
    if (!obj || !obj.metadata?.isEnvironmentObject) return;
    
    setEnvironmentIntensity(scene.environmentIntensity || 1.0);
    
    // Update skybox visibility based on selected object
    setSkyboxVisible(obj.isEnabled());
    
    // Load skybox-specific settings from metadata
    if (obj.metadata?.skyboxSettings) {
      const settings = obj.metadata.skyboxSettings;
      setSkyboxBrightness(settings.brightness || 1.0);
      
      // Load skybox color from stored metadata since we use texture now
      if (settings.color) {
        setSkyboxColor(settings.color);
      }
    }
    
    // Load fog settings from scene
    if (scene.fogEnabled !== undefined) {
      setFogEnabled(scene.fogEnabled);
    }
    if (scene.fogColor) {
      const color = scene.fogColor;
      const r = Math.round(color.r * 255).toString(16).padStart(2, '0');
      const g = Math.round(color.g * 255).toString(16).padStart(2, '0');
      const b = Math.round(color.b * 255).toString(16).padStart(2, '0');
      setFogColor(`#${r}${g}${b}`);
    }
    if (scene.fogDensity !== undefined) {
      setFogDensity(scene.fogDensity);
    }
    if (scene.fogStart !== undefined) {
      setFogStart(scene.fogStart);
    }
    if (scene.fogEnd !== undefined) {
      setFogEnd(scene.fogEnd);
    }
  });

  // Reactive effect for skybox color changes
  createEffect(() => {
    const obj = selectedObject();
    if (!obj || !obj.metadata?.isEnvironmentObject || !obj.material) return;
    
    const color = skyboxColor();
    const r = parseInt(color.slice(1, 3), 16) / 255;
    const g = parseInt(color.slice(3, 5), 16) / 255;
    const b = parseInt(color.slice(5, 7), 16) / 255;
    
    console.log('🎨 Updating skybox color:', color, { r, g, b });
    console.log('🎨 Material type:', obj.material.constructor.name);
    
    // Update skybox texture color for PBR reflections
    if (obj.material.reflectionTexture && obj.material.reflectionTexture.getContext) {
      const texture = obj.material.reflectionTexture;
      const context = texture.getContext();
      
      // Update texture with new color
      context.fillStyle = color;
      context.fillRect(0, 0, texture.getSize().width, texture.getSize().height);
      texture.update();
      
      // Update scene environment texture for PBR
      if (obj.material._scene) {
        obj.material._scene.environmentTexture = texture;
        obj.material._scene.markAllMaterialsAsDirty();
      }
      
      console.log('🎨 Updated skybox texture color:', color);
    }
    
    // Store in metadata
    if (!obj.metadata.skyboxSettings) {
      obj.metadata.skyboxSettings = {};
    }
    obj.metadata.skyboxSettings.color = color;
  });

  // Reactive effect for environment intensity changes
  createEffect(() => {
    const scene = renderStore.scene;
    if (!scene) return;
    
    const intensity = environmentIntensity();
    console.log('🌍 Environment intensity change:', intensity);
    
    // Environment intensity affects IBL (Image Based Lighting) for PBR materials
    if (scene.environmentTexture) {
      scene.environmentIntensity = intensity;
      console.log('🌍 Set IBL environment intensity:', intensity);
    }
    
    // Also affect scene's overall lighting for non-PBR materials
    scene.ambientColor = scene.ambientColor || new Color3(0.2, 0.2, 0.2);
    const baseAmbient = 0.2;
    scene.ambientColor.r = baseAmbient * intensity;
    scene.ambientColor.g = baseAmbient * intensity;
    scene.ambientColor.b = baseAmbient * intensity;
    
    console.log('🌍 Applied environment intensity:', intensity, 'IBL enabled:', !!scene.environmentTexture);
  });

  // Reactive effect for skybox brightness changes
  createEffect(() => {
    const obj = selectedObject();
    if (!obj || !obj.metadata?.isEnvironmentObject || !obj.material) return;
    
    const material = obj.material;
    const brightness = skyboxBrightness();
    const color = skyboxColor();
    
    console.log('💡 Updating skybox brightness:', brightness, 'with color:', color);
    
    // Apply brightness to texture by adjusting color intensity
    if (material.reflectionTexture && material.reflectionTexture.getContext) {
      const texture = material.reflectionTexture;
      const context = texture.getContext();
      
      // Calculate brightened color
      const r = parseInt(color.slice(1, 3), 16);
      const g = parseInt(color.slice(3, 5), 16);
      const b = parseInt(color.slice(5, 7), 16);
      
      // Apply brightness multiplier and clamp to 255
      const brightR = Math.min(255, Math.round(r * brightness));
      const brightG = Math.min(255, Math.round(g * brightness));
      const brightB = Math.min(255, Math.round(b * brightness));
      
      const brightenedColor = `rgb(${brightR}, ${brightG}, ${brightB})`;
      
      // Update texture
      context.fillStyle = brightenedColor;
      context.fillRect(0, 0, texture.getSize().width, texture.getSize().height);
      texture.update();
      
      console.log('💡 Applied brightness to texture:', brightenedColor);
    }
    
    // Update metadata
    if (obj.metadata?.skyboxSettings) {
      obj.metadata.skyboxSettings.brightness = brightness;
    }
  });

  // Reactive effect for skybox visibility changes
  createEffect(() => {
    const obj = selectedObject();
    if (!obj || !obj.metadata?.isEnvironmentObject) return;
    
    console.log('🔄 Skybox visibility change:', skyboxVisible(), 'Current enabled:', obj.isEnabled());
    console.log('🔄 Object details:', {
      name: obj.name,
      isVisible: obj.isVisible,
      renderingGroupId: obj.renderingGroupId,
      hasMaterial: !!obj.material,
      materialType: obj.material?.constructor.name
    });
    
    // For skyboxes, we need to use different visibility methods
    const isVisible = skyboxVisible();
    
    // Method 1: Set enabled state
    obj.setEnabled(isVisible);
    
    // Method 2: Set visibility directly
    obj.isVisible = isVisible;
    
    // Method 3: For skybox, also control rendering group
    if (!isVisible) {
      obj.renderingGroupId = -1; // Hide from rendering
    } else {
      obj.renderingGroupId = 0; // Back to normal rendering
    }
    
    // Method 4: Material alpha control
    if (obj.material) {
      obj.material.alpha = isVisible ? 1.0 : 0.0;
    }
    
    console.log('✅ Applied visibility:', isVisible, 'Enabled:', obj.isEnabled(), 'Visible:', obj.isVisible, 'RenderGroup:', obj.renderingGroupId);
  });

  // Reactive effect for skybox image changes
  createEffect(() => {
    const obj = selectedObject();
    const imageData = skyboxImage();
    
    if (!obj || !obj.metadata?.isEnvironmentObject || !obj.material) return;
    
    if (imageData) {
      const scene = obj.material._scene;
      
      // Check if it's HDR data using native BabylonJS support
      if (typeof imageData === 'object' && imageData.type === 'hdr') {
        console.log('🌍 Loading HDR with native BabylonJS support:', imageData.name);
        
        // Wait for texture to load before applying
        let loadHandled = false;
        
        try {
          // Try approach 1: HDRCubeTexture with proper material setup
          console.log('🌍 Trying HDR loading approach 1: HDRCubeTexture with proper setup');
          
          const hdrTexture = new HDRCubeTexture(imageData.url, scene, 512);
          
          console.log('🌍 HDR texture created:', {
            url: imageData.url,
            isReady: hdrTexture.isReady(),
            hasObservable: !!hdrTexture.onLoadObservable
          });
          
          // Add error handling for load failures
          if (hdrTexture.onErrorObservable) {
            hdrTexture.onErrorObservable.add((error) => {
              console.error('❌ HDR texture load error:', error);
            });
          }
          
          // Test if we can fetch the URL directly
          fetch(imageData.url)
            .then(response => {
              console.log('🌍 Direct fetch test result:', {
                ok: response.ok,
                status: response.status,
                contentType: response.headers.get('content-type'),
                contentLength: response.headers.get('content-length')
              });
              return response.blob();
            })
            .then(blob => {
              console.log('🌍 HDR file blob size:', blob.size, 'type:', blob.type);
            })
            .catch(error => {
              console.error('❌ Direct fetch failed:', error);
            });
          hdrTexture.onLoadObservable.addOnce(() => {
            loadHandled = true;
            console.log('🌍 HDR texture loaded via observable, applying to skybox...');
            applyHDRTexture(hdrTexture, scene, obj);
          });
          
          // Multiple fallbacks to ensure HDR gets applied
          const checkAndApplyHDR = () => {
            if (!loadHandled && hdrTexture.isReady()) {
              loadHandled = true;
              console.log('🔄 HDR texture ready via polling, applying fallback setup...');
              applyHDRTexture(hdrTexture, scene, obj);
            } else if (!loadHandled) {
              console.log('🌍 HDR texture not ready yet, will retry...');
            }
          };
          
          // Check multiple times in case observable doesn't fire
          setTimeout(checkAndApplyHDR, 100);
          setTimeout(checkAndApplyHDR, 500);
          setTimeout(checkAndApplyHDR, 1000);
          setTimeout(checkAndApplyHDR, 2000);
          
          // Function to apply HDR texture to skybox
          const applyHDRTexture = (texture, scene, obj) => {
            console.log('🌍 Applying HDR texture to skybox...');
            console.log('🌍 HDR texture details:', {
              isReady: texture.isReady(),
              size: texture.getSize ? texture.getSize() : 'unknown',
              url: texture.url
            });
            
            // Set environment texture first for PBR lighting
            scene.environmentTexture = texture;
            scene.environmentIntensity = 1.0;
            
            // Apply HDR texture to skybox material with proper settings
            obj.material.reflectionTexture = texture;
            obj.material.reflectionTexture.coordinatesMode = Texture.SKYBOX_MODE;
            
            // Critical: Set material to be fully reflective (no diffuse/specular)
            obj.material.diffuseColor = new Color3(0, 0, 0);
            obj.material.specularColor = new Color3(0, 0, 0);
            obj.material.disableLighting = true;
            
            // Force the material to use the reflection texture
            obj.material.reflectionFresnelParameters = null;
            
            console.log('🌍 Material setup:', {
              hasReflectionTexture: !!obj.material.reflectionTexture,
              coordinatesMode: obj.material.reflectionTexture?.coordinatesMode,
              disableLighting: obj.material.disableLighting,
              materialType: obj.material.constructor.name
            });
            
            scene.markAllMaterialsAsDirty();
            console.log('✅ HDR texture applied with proper skybox setup');
          };
          
        } catch (error) {
          console.error('❌ HDRCubeTexture approach failed:', error);
        }
        
        // Try approach 2: Load HDR as ArrayBuffer and create texture from data
        setTimeout(() => {
          if (!loadHandled) {
            console.log('🔄 HDR texture failed to load, trying ArrayBuffer approach...');
            
            fetch(imageData.url)
              .then(response => response.arrayBuffer())
              .then(arrayBuffer => {
                console.log('🌍 HDR ArrayBuffer loaded, size:', arrayBuffer.byteLength);
                
                // Create HDR texture from ArrayBuffer
                const hdrFromBuffer = new HDRCubeTexture(null, scene, 512);
                
                // BabylonJS expects HDR data in a specific format
                // Let's try creating a blob URL instead
                const blob = new Blob([arrayBuffer], { type: 'image/vnd.radiance' });
                const blobUrl = URL.createObjectURL(blob);
                
                console.log('🌍 Created blob URL for HDR:', blobUrl);
                
                const hdrFromBlob = new HDRCubeTexture(blobUrl, scene, 512);
                hdrFromBlob.onLoadObservable.addOnce(() => {
                  loadHandled = true;
                  console.log('🌍 HDR from blob loaded successfully!');
                  applyHDRTexture(hdrFromBlob, scene, obj);
                  
                  // Clean up blob URL
                  URL.revokeObjectURL(blobUrl);
                });
                
                // Fallback to regular texture if blob approach also fails
                setTimeout(() => {
                  if (!loadHandled) {
                    console.log('🔄 Blob approach failed, trying regular texture...');
                    try {
                      const fallbackTexture = new Texture(imageData.url, scene);
                      fallbackTexture.onLoadObservable.addOnce(() => {
                        console.log('🌍 Regular texture loaded, applying as skybox...');
                        loadHandled = true;
                        
                        obj.material.reflectionTexture = fallbackTexture;
                        obj.material.reflectionTexture.coordinatesMode = Texture.SKYBOX_MODE;
                        obj.material.disableLighting = true;
                        obj.material.diffuseColor = new Color3(0, 0, 0);
                        obj.material.specularColor = new Color3(0, 0, 0);
                        
                        scene.environmentTexture = fallbackTexture;
                        scene.markAllMaterialsAsDirty();
                        
                        console.log('✅ Applied HDR as regular texture');
                      });
                    } catch (fallbackError) {
                      console.error('❌ Regular texture approach also failed:', fallbackError);
                    }
                  }
                }, 2000);
              })
              .catch(error => {
                console.error('❌ Failed to load HDR as ArrayBuffer:', error);
              });
          }
        }, 3000);
      } else {
        // Handle regular image files (string URLs or objects with url property)
        const imageUrl = typeof imageData === 'string' ? imageData : imageData.url;
        
        console.log('📸 Loading regular image texture:', imageUrl);
        const imageTexture = new Texture(imageUrl, scene);
        obj.material.reflectionTexture = imageTexture;
        obj.material.reflectionTexture.coordinatesMode = Texture.SKYBOX_MODE;
        scene.environmentTexture = imageTexture;
        scene.markAllMaterialsAsDirty();
      }
    } else {
      // Revert to solid color texture
      const color = skyboxColor();
      const brightness = skyboxBrightness();
      
      if (obj.material.reflectionTexture && obj.material.reflectionTexture.getContext) {
        const texture = obj.material.reflectionTexture;
        const context = texture.getContext();
        
        // Calculate brightened color
        const r = parseInt(color.slice(1, 3), 16);
        const g = parseInt(color.slice(3, 5), 16);
        const b = parseInt(color.slice(5, 7), 16);
        
        const brightR = Math.min(255, Math.round(r * brightness));
        const brightG = Math.min(255, Math.round(g * brightness));
        const brightB = Math.min(255, Math.round(b * brightness));
        
        const brightenedColor = `rgb(${brightR}, ${brightG}, ${brightB})`;
        
        context.fillStyle = brightenedColor;
        context.fillRect(0, 0, texture.getSize().width, texture.getSize().height);
        texture.update();
        
        console.log('🎨 Reverted to solid color texture:', brightenedColor);
      }
    }
  });

  // Reactive effects for fog controls
  createEffect(() => {
    const scene = renderStore.scene;
    if (!scene) return;
    
    scene.fogEnabled = fogEnabled();
    console.log('🌫️ Fog enabled:', fogEnabled());
  });

  createEffect(() => {
    const scene = renderStore.scene;
    if (!scene) return;
    
    const color = fogColor();
    const r = parseInt(color.slice(1, 3), 16) / 255;
    const g = parseInt(color.slice(3, 5), 16) / 255;
    const b = parseInt(color.slice(5, 7), 16) / 255;
    
    scene.fogColor = new Color3(r, g, b);
    console.log('🌫️ Fog color:', scene.fogColor);
  });

  createEffect(() => {
    const scene = renderStore.scene;
    if (!scene) return;
    
    scene.fogDensity = fogDensity();
    console.log('🌫️ Fog density:', fogDensity());
  });

  createEffect(() => {
    const scene = renderStore.scene;
    if (!scene) return;
    
    scene.fogStart = fogStart();
    scene.fogEnd = fogEnd();
    console.log('🌫️ Fog range:', fogStart(), 'to', fogEnd());
  });

  return (
    <div class="h-full overflow-y-auto p-4 space-y-4 bg-base-100">
      {/* Header */}
      <div class="flex items-center space-x-2 pb-2 border-b border-base-300">
        <IconSun class="w-4 h-4 text-primary" />
        <h3 class="text-sm font-medium text-base-content">Environment Settings</h3>
      </div>
      
      {/* Skybox Settings Section */}
      <div class="card bg-base-200 shadow-sm">
        <div class="card-body p-4">
          <h4 class="card-title text-sm">Skybox Settings</h4>
          
          <div class="space-y-3">
            <div class="form-control">
              <div class="flex items-center justify-between">
                <label class="label-text text-xs">Visible</label>
                <input
                  type="checkbox"
                  class="toggle toggle-primary toggle-sm"
                  checked={skyboxVisible()}
                  onChange={(e) => setSkyboxVisible(e.target.checked)}
                />
              </div>
            </div>

            <div class="form-control">
              <label class="label-text text-xs mb-1">Color</label>
              <div class="flex items-center space-x-2">
                <input 
                  type="color" 
                  class="w-10 h-8 rounded border border-base-300 cursor-pointer"
                  value={skyboxColor()}
                  onInput={(e) => setSkyboxColor(e.target.value)}
                  disabled={skyboxImage() !== null}
                />
                <input 
                  type="text" 
                  class="input input-xs input-bordered flex-1 font-mono"
                  value={skyboxColor()}
                  onInput={(e) => setSkyboxColor(e.target.value)}
                  placeholder="#87CEEB"
                  disabled={skyboxImage() !== null}
                />
              </div>
              {skyboxImage() && (
                <div class="text-xs text-base-content/50 mt-1">Color disabled when using image</div>
              )}
            </div>
            
            <div class="form-control">
              <label class="label-text text-xs mb-1">Skybox Image</label>
              <div 
                class={`border-2 border-dashed rounded-lg p-4 text-center transition-colors cursor-pointer ${
                  isDragging() 
                    ? 'border-primary bg-primary/10' 
                    : skyboxImage() 
                      ? 'border-success bg-success/10' 
                      : 'border-base-300 hover:border-base-400'
                }`}
                onDragOver={handleDragOver}
                onDragLeave={handleDragLeave}
                onDrop={handleDrop}
                onClick={() => document.getElementById('skybox-file-input').click()}
              >
                {skyboxImage() ? (
                  <div class="space-y-2">
                    <div class="text-xs text-success font-medium">Image Loaded ✓</div>
                    <img 
                      src={skyboxImage()} 
                      alt="Skybox preview" 
                      class="w-16 h-16 object-cover rounded mx-auto"
                    />
                    <button 
                      class="btn btn-xs btn-outline btn-error"
                      onClick={(e) => { e.stopPropagation(); clearSkyboxImage(); }}
                    >
                      Remove
                    </button>
                  </div>
                ) : (
                  <div class="space-y-1">
                    <div class="text-xs text-base-content/60">
                      {isDragging() ? 'Drop image here' : 'Drag & drop skybox image'}
                    </div>
                    <div class="text-xs text-base-content/40">
                      or click to browse
                    </div>
                    <div class="text-xs text-base-content/30">
                      JPG, PNG, WebP, HDR, EXR supported
                    </div>
                  </div>
                )}
              </div>
              <input
                id="skybox-file-input"
                type="file"
                accept="image/*,.hdr,.exr"
                class="hidden"
                onChange={handleFileChange}
              />
            </div>

            <div class="form-control">
              <div class="flex items-center justify-between mb-1">
                <label class="label-text text-xs">Brightness</label>
                <span class="text-xs text-base-content/60">{skyboxBrightness().toFixed(2)}</span>
              </div>
              <input
                type="range"
                min="0"
                max="3"
                step="0.01"
                value={skyboxBrightness()}
                onInput={(e) => setSkyboxBrightness(parseFloat(e.target.value))}
                class="range range-primary range-xs"
                disabled={skyboxImage() !== null}
              />
              {skyboxImage() && (
                <div class="text-xs text-base-content/50 mt-1">Brightness disabled when using image</div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Environment Lighting Section */}
      <div class="card bg-base-200 shadow-sm">
        <div class="card-body p-4">
          <h4 class="card-title text-sm">Environment Lighting</h4>
          
          <div class="space-y-3">
            <div class="form-control">
              <div class="flex items-center justify-between mb-1">
                <label class="label-text text-xs">IBL Intensity</label>
                <span class="text-xs text-base-content/60">{environmentIntensity().toFixed(2)}</span>
              </div>
              <input
                type="range"
                min="0"
                max="3"
                step="0.01"
                value={environmentIntensity()}
                onInput={(e) => setEnvironmentIntensity(parseFloat(e.target.value))}
                class="range range-primary range-xs"
              />
              <div class="text-xs text-base-content/50 mt-1">Controls PBR reflections and ambient lighting</div>
            </div>
          </div>
        </div>
      </div>

      {/* Fog Settings Section */}
      <div class="card bg-base-200 shadow-sm">
        <div class="card-body p-4">
          <h4 class="card-title text-sm">Fog Settings</h4>
          
          <div class="space-y-3">
            <div class="form-control">
              <div class="flex items-center justify-between">
                <label class="label-text text-xs">Enable Fog</label>
                <input
                  type="checkbox"
                  class="toggle toggle-primary toggle-sm"
                  checked={fogEnabled()}
                  onChange={(e) => setFogEnabled(e.target.checked)}
                />
              </div>
            </div>

            <div class="form-control">
              <label class="label-text text-xs mb-1">Fog Color</label>
              <div class="flex items-center space-x-2">
                <input 
                  type="color" 
                  class="w-10 h-8 rounded border border-base-300 cursor-pointer"
                  value={fogColor()}
                  onInput={(e) => setFogColor(e.target.value)}
                  disabled={!fogEnabled()}
                />
                <input 
                  type="text" 
                  class="input input-xs input-bordered flex-1 font-mono"
                  value={fogColor()}
                  onInput={(e) => setFogColor(e.target.value)}
                  placeholder="#CCCCCC"
                  disabled={!fogEnabled()}
                />
              </div>
            </div>
            
            <div class="form-control">
              <div class="flex items-center justify-between mb-1">
                <label class="label-text text-xs">Density</label>
                <span class="text-xs text-base-content/60">{fogDensity().toFixed(3)}</span>
              </div>
              <input
                type="range"
                min="0"
                max="0.1"
                step="0.001"
                value={fogDensity()}
                onInput={(e) => setFogDensity(parseFloat(e.target.value))}
                class="range range-primary range-xs"
                disabled={!fogEnabled()}
              />
            </div>

            <div class="grid grid-cols-2 gap-2">
              <div class="form-control">
                <label class="label-text text-xs mb-1">Start Distance</label>
                <input
                  type="number"
                  class="input input-xs input-bordered"
                  value={fogStart()}
                  onInput={(e) => setFogStart(parseFloat(e.target.value))}
                  min="0"
                  step="1"
                  disabled={!fogEnabled()}
                />
              </div>
              <div class="form-control">
                <label class="label-text text-xs mb-1">End Distance</label>
                <input
                  type="number"
                  class="input input-xs input-bordered"
                  value={fogEnd()}
                  onInput={(e) => setFogEnd(parseFloat(e.target.value))}
                  min="1"
                  step="1"
                  disabled={!fogEnabled()}
                />
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default EnvironmentPanel;