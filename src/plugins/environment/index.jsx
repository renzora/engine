import { createPlugin } from '@/api/plugin';
import { IconSun } from '@tabler/icons-solidjs';
import EnvironmentPanel from './EnvironmentPanel.jsx';
import { CreateSphere } from '@babylonjs/core/Meshes/Builders/sphereBuilder';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { Color3 } from '@babylonjs/core/Maths/math.color';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { Texture } from '@babylonjs/core/Materials/Textures/texture';
import { DynamicTexture } from '@babylonjs/core/Materials/Textures/dynamicTexture';
import { CubeTexture } from '@babylonjs/core/Materials/Textures/cubeTexture';

// Handle skybox creation
const handleCreateSkybox = async () => {
  console.log('🌍 Environment plugin received skybox creation event');
  try {
    // Get the current scene from render store
    const { renderStore, renderActions } = await import('@/render/store');
    const scene = renderStore.scene;
    console.log('🌍 Scene available:', !!scene);
    
    if (!scene) {
      console.error('❌ No active scene to create skybox in');
      return;
    }

    // Check if skybox already exists
    const existingSkybox = scene.meshes.find(mesh => 
      mesh.name.includes('skybox') || mesh.name.includes('skyBox')
    );
    
    if (existingSkybox) {
      console.warn('⚠️ Skybox already exists in scene');
      // Select the existing skybox instead
      renderActions.selectObject(existingSkybox);
      return;
    }

    // Create skybox with default settings
    const skyboxName = 'Skybox';
    const skybox = CreateSphere(skyboxName, { diameter: 1000 }, scene);
    skybox.infiniteDistance = true;
    skybox.renderingGroupId = 0; // Render skybox first
    skybox.receiveShadows = false; // Skybox should not receive shadows
    
    // Make sure skybox is visible and enabled
    skybox.isVisible = true;
    skybox.setEnabled(true);
    
    // Mark as environment object for hierarchy grouping
    skybox.metadata = {
      isEnvironmentObject: true,
      skyboxSettings: {
        turbidity: 10,
        luminance: 1.0,
        inclination: 0.5,
        azimuth: 0.25,
        cloudsEnabled: true,
        cloudSize: 25,
        cloudDensity: 0.6,
        cloudOpacity: 0.8,
        color: '#87CEEB' // Default sky blue
      }
    };

    // Create proper PBR skybox material for reflections
    const skyboxMaterial = new StandardMaterial(skyboxName + 'Material', scene);
    skyboxMaterial.backFaceCulling = false;
    skyboxMaterial.disableLighting = true;
    
    // Create a simple colored texture for the skybox
    const skyTexture = new DynamicTexture('skyboxTexture', { width: 512, height: 512 }, scene);
    const textureContext = skyTexture.getContext();
    
    // Fill with sky blue color
    textureContext.fillStyle = '#7fccff'; // Sky blue
    textureContext.fillRect(0, 0, 512, 512);
    skyTexture.update();
    
    // Set as reflection texture for proper skybox behavior
    skyboxMaterial.reflectionTexture = skyTexture;
    skyboxMaterial.reflectionTexture.coordinatesMode = Texture.SKYBOX_MODE;
    skyboxMaterial.diffuseColor = new Color3(0, 0, 0); // No diffuse
    skyboxMaterial.specularColor = new Color3(0, 0, 0); // No specular
    
    skybox.material = skyboxMaterial;
    
    // Set scene environment texture for PBR reflections
    scene.environmentTexture = skyTexture;
    scene.environmentIntensity = 1.0;
    
    console.log('🌍 Created skybox with material:', {
      name: skyboxName,
      diameter: 1000,
      materialType: skyboxMaterial.constructor.name,
      diffuseColor: skyboxMaterial.diffuseColor,
      enabled: skybox.isEnabled(),
      visible: skybox.isVisible,
      renderingGroup: skybox.renderingGroupId,
      infiniteDistance: skybox.infiniteDistance,
      position: skybox.position,
      scaling: skybox.scaling
    });
    
    // Additional debugging
    console.log('🌍 Material details:', {
      backFaceCulling: skyboxMaterial.backFaceCulling,
      disableLighting: skyboxMaterial.disableLighting,
      alpha: skyboxMaterial.alpha,
      diffuseColor: skyboxMaterial.diffuseColor
    });

    // Add to scene hierarchy and select it
    renderActions.addObject(skybox);
    renderActions.selectObject(skybox);
    
    // Mark scene as modified
    const { sceneManager } = await import('@/api/scene/SceneManager.js');
    sceneManager.markAsModified();

    console.log('✅ Skybox created successfully');
    
    // Show success message
    const { editorActions } = await import('@/layout/stores/EditorStore');
    editorActions.addConsoleMessage('Skybox created successfully', 'success');

  } catch (error) {
    console.error('❌ Failed to create skybox:', error);
    
    // Show error message
    try {
      const { editorActions } = await import('@/layout/stores/EditorStore');
      editorActions.addConsoleMessage('Failed to create skybox', 'error');
    } catch (e) {
      // Ignore if editor store is not available
    }
  }
};

export default createPlugin({
  id: 'environment',
  name: 'Environment Plugin',
  version: '1.0.0',
  description: 'Environment controls for skybox, fog, and scene atmosphere',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('🌍 Environment plugin initializing...');
  },

  async onStart(api) {
    console.log('🌍 Environment plugin starting...');
    
    // Register the environment panel as a tab
    api.tab('environment', {
      title: 'Environment',
      component: EnvironmentPanel,
      icon: IconSun,
      order: 5,
      condition: (selectedObject) => {
        return selectedObject && selectedObject.metadata?.isEnvironmentObject;
      }
    });

    // Listen for skybox creation events from the menu
    document.addEventListener('engine:create-skybox', handleCreateSkybox);

    console.log('🌍 Environment plugin started successfully');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    // Remove event listeners
    document.removeEventListener('engine:create-skybox', handleCreateSkybox);
    console.log('🗑️ Environment plugin stopped');
  },

  async onDispose() {
    console.log('🗑️ Environment plugin disposed');
  }
});