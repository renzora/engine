import { createSignal, createEffect } from 'solid-js';
import { PostProcess } from '@babylonjs/core/PostProcesses/postProcess';
import { Effect } from '@babylonjs/core/Materials/effect';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { renderStore } from '@/render/store.jsx';

// Helper function to mark scene as modified
const markSceneAsModified = () => {
  try {
    import('@/api/scene/SceneManager.js').then(({ sceneManager }) => {
      sceneManager.markAsModified();
    }).catch(err => {
      console.warn('Failed to mark scene as modified:', err);
    });
  } catch (error) {
    console.warn('Failed to import SceneManager:', error);
  }
};

// Camera settings store
const [cameraSettings, setCameraSettings] = createSignal({
  fov: 60,
  vignette: {
    enabled: false,
    amount: 0.5,
    color: [0, 0, 0]
  }
});

export const cameraActions = {
  setFOV: (fov) => {
    setCameraSettings(prev => ({ ...prev, fov }));
    markSceneAsModified();
  },
  
  setVignetteEnabled: (enabled) => {
    setCameraSettings(prev => ({
      ...prev,
      vignette: { ...prev.vignette, enabled }
    }));
    markSceneAsModified();
  },
  
  setVignetteAmount: (amount) => {
    setCameraSettings(prev => ({
      ...prev,
      vignette: { ...prev.vignette, amount }
    }));
    markSceneAsModified();
  },
  
  setVignetteColor: (color) => {
    setCameraSettings(prev => ({
      ...prev,
      vignette: { ...prev.vignette, color }
    }));
    markSceneAsModified();
  },
  
  resetToDefaults: () => {
    setCameraSettings({
      fov: 60,
      vignette: {
        enabled: false,
        amount: 0.5,
        color: [0, 0, 0]
      }
    });
    markSceneAsModified();
  }
};

// Global vignette post-process management
let vignettePostProcess = null;

// Create or destroy vignette post-process based on enabled state
const createVignettePostProcess = () => {
  const scene = renderStore.scene;
  const camera = scene?.activeCamera;
  
  if (!scene || !camera) return;
  
  // Clean up existing vignette first
  cleanupVignette();
  
  try {
    // Register the shader with Babylon.js effect system
    const shaderName = 'cameraVignette';
    const fragmentShader = `
      precision highp float;
      varying vec2 vUV;
      uniform sampler2D textureSampler;
      uniform float intensity;
      uniform vec3 vignetteColor;
      
      void main() {
        vec4 color = texture2D(textureSampler, vUV);
        vec2 center = vec2(0.5, 0.5);
        float distance = length(vUV - center);
        float vignetteMask = 1.0 - smoothstep(0.0, 1.0, distance * intensity);
        
        // Mix original color with vignette color based on the mask
        vec3 finalColor = mix(vignetteColor, color.rgb, vignetteMask);
        gl_FragColor = vec4(finalColor, color.a);
      }
    `;
    
    // Register the shader if not already registered
    if (!Effect.ShadersStore[shaderName + 'FragmentShader']) {
      Effect.ShadersStore[shaderName + 'FragmentShader'] = fragmentShader;
    }
    
    vignettePostProcess = new PostProcess(
      'camera_vignette',
      shaderName,
      ['intensity', 'vignetteColor'],
      null,
      1.0,
      camera
    );
    
    // Set initial uniforms and keep reference for updates
    if (vignettePostProcess) {
      updateVignetteUniforms();
    }
  } catch (error) {
    console.error('Failed to create vignette post-process:', error);
    vignettePostProcess = null;
  }
};

// Update only the uniform values without recreating the post-process
const updateVignetteUniforms = () => {
  if (!vignettePostProcess) return;
  
  const currentSettings = cameraSettings();
  
  // Update uniforms using onApply callback
  vignettePostProcess.onApply = (effect) => {
    if (effect && effect.setFloat && effect.setVector3) {
      effect.setFloat('intensity', currentSettings.vignette.amount * 2);
      effect.setVector3('vignetteColor', new Vector3(
        currentSettings.vignette.color[0],
        currentSettings.vignette.color[1],
        currentSettings.vignette.color[2]
      ));
    }
  };
};

// Track previous vignette enabled state to avoid unnecessary recreations
let previousVignetteEnabled = false;

// Auto-initialize camera effects when this module is imported
// This will be executed within the plugin system's createRoot context
createEffect(() => {
  const scene = renderStore.scene;
  const camera = scene?.activeCamera;
  const currentSettings = cameraSettings();
  
  if (!scene || !camera) return;
  
  // Update FOV
  camera.fov = (currentSettings.fov * Math.PI) / 180;
  
  // Handle vignette enable/disable (recreate post-process)
  if (currentSettings.vignette.enabled !== previousVignetteEnabled) {
    previousVignetteEnabled = currentSettings.vignette.enabled;
    
    if (currentSettings.vignette.enabled) {
      createVignettePostProcess();
    } else {
      cleanupVignette();
    }
  } else if (currentSettings.vignette.enabled && vignettePostProcess) {
    // Vignette is enabled and post-process exists, just update uniforms
    updateVignetteUniforms();
  }
});

// Function to clean up vignette when needed
export const cleanupVignette = () => {
  if (vignettePostProcess) {
    try {
      vignettePostProcess.dispose();
    } catch (e) {
      console.warn('Failed to dispose vignette post-process:', e);
    }
    vignettePostProcess = null;
  }
};

export { cameraSettings };