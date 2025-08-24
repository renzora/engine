import { Engine } from '@babylonjs/core';
import { Scene } from '@babylonjs/core';
import { ArcRotateCamera } from '@babylonjs/core';
import { HemisphericLight } from '@babylonjs/core';
import { DirectionalLight } from '@babylonjs/core';
import { Vector3 } from '@babylonjs/core';
import { Color3, Color4 } from '@babylonjs/core';
import { SceneLoader } from '@babylonjs/core';
import { MeshBuilder } from '@babylonjs/core';
import { StandardMaterial } from '@babylonjs/core';
import { GridMaterial } from '@babylonjs/materials';
import { ShadowGenerator } from '@babylonjs/core';
import { CubeTexture } from '@babylonjs/core';
import '@babylonjs/loaders';
import '@babylonjs/materials';

// Import skeleton utilities
import { Skeleton } from '@babylonjs/core';

class ModelThumbnailGenerator {
  constructor() {
    this.thumbnailCache = new Map();
    this.canvas = null;
    this.engine = null;
    this.scene = null;
    this.isInitialized = false;
  }

  async initialize() {
    if (this.isInitialized) return;

    // Create a hidden canvas for rendering
    this.canvas = document.createElement('canvas');
    this.canvas.width = 512; // Higher resolution for better quality
    this.canvas.height = 512;
    this.canvas.style.display = 'none';
    document.body.appendChild(this.canvas);

    // Create Babylon.js engine and scene
    this.engine = new Engine(this.canvas, true, {
      preserveDrawingBuffer: true,
      stencil: true,
      antialias: true
    });

    this.scene = new Scene(this.engine);
    
    // Create visible gradient sky background
    const skyMaterial = new StandardMaterial('skyMaterial', this.scene);
    skyMaterial.diffuseColor = new Color3(0.25, 0.35, 0.55);
    skyMaterial.emissiveColor = new Color3(0.2, 0.3, 0.5);
    skyMaterial.backFaceCulling = false;
    
    const skybox = MeshBuilder.CreateSphere('skybox', { diameter: 200 }, this.scene);
    skybox.material = skyMaterial;
    
    // Set clear color to sky color
    this.scene.clearColor = new Color4(0.2, 0.3, 0.5, 1);
    this.scene.ambientColor = new Color3(0.4, 0.4, 0.45);

    // Setup camera - facing front of model
    this.camera = new ArcRotateCamera(
      'thumbnailCamera',
      Math.PI * 0.75, // 135 degrees - front-right view
      Math.PI / 2.5,  // 72 degrees - slightly more elevated angle (panned down)
      5,
      Vector3.Zero(),
      this.scene
    );
    this.camera.attachControl(this.canvas, false);
    this.camera.wheelPrecision = 50;
    this.camera.minZ = 0.01;
    this.camera.maxZ = 1000;

    // Setup enhanced lighting - brighter for dark background
    const hemiLight = new HemisphericLight(
      'hemiLight',
      new Vector3(0, 1, 0),
      this.scene
    );
    hemiLight.intensity = 1.0;
    hemiLight.groundColor = new Color3(0.4, 0.4, 0.5);
    hemiLight.specular = new Color3(0.4, 0.4, 0.4);

    // Key light (main directional light with shadows)
    this.dirLight = new DirectionalLight(
      'dirLight',
      new Vector3(-0.5, -1, -0.5),
      this.scene
    );
    this.dirLight.intensity = 1.2;
    this.dirLight.position = new Vector3(10, 20, 10);

    // Fill light (softer, from opposite direction)
    const fillLight = new DirectionalLight(
      'fillLight',
      new Vector3(0.5, -0.5, 0.5),
      this.scene
    );
    fillLight.intensity = 0.6;
    fillLight.specular = new Color3(0.3, 0.3, 0.3);

    // Rim light (back light for edge definition)
    const rimLight = new DirectionalLight(
      'rimLight',
      new Vector3(0, -0.5, 1),
      this.scene
    );
    rimLight.intensity = 0.4;
    rimLight.diffuse = new Color3(1, 1, 1.2);

    // Create ground with grid
    this.ground = MeshBuilder.CreateGround('ground', {
      width: 20,
      height: 20,
      subdivisions: 2
    }, this.scene);
    this.ground.position.y = -0.01; // Slightly below zero to avoid z-fighting

    // Create grid material for ground - more visible grid lines
    const gridMaterial = new GridMaterial('gridMaterial', this.scene);
    gridMaterial.majorUnitFrequency = 5;
    gridMaterial.minorUnitVisibility = 0.45;
    gridMaterial.gridRatio = 1;
    gridMaterial.backFaceCulling = false;
    gridMaterial.mainColor = new Color3(0.08, 0.09, 0.11);
    gridMaterial.lineColor = new Color3(0.3, 0.32, 0.36);
    gridMaterial.opacity = 1.0;
    this.ground.material = gridMaterial;

    // Setup shadow generator
    this.shadowGenerator = new ShadowGenerator(1024, this.dirLight);
    this.shadowGenerator.useBlurExponentialShadowMap = true;
    this.shadowGenerator.blurKernel = 32;
    this.shadowGenerator.setDarkness(0.3);

    // Enable shadows on ground
    this.ground.receiveShadows = true;

    this.isInitialized = true;
  }

  async generateThumbnail(modelUrl, modelPath) {
    // Check cache first
    const cacheKey = modelPath || modelUrl;
    if (this.thumbnailCache.has(cacheKey)) {
      return this.thumbnailCache.get(cacheKey);
    }

    try {
      await this.initialize();

      // Clear previous meshes (except ground)
      const previousMeshes = this.scene.meshes.filter(mesh => mesh.name !== 'ground');
      previousMeshes.forEach(mesh => {
        if (mesh && !mesh.isDisposed()) {
          // Remove from shadow generator if it was added
          if (this.shadowGenerator.getShadowMap().renderList.includes(mesh)) {
            this.shadowGenerator.removeShadowCaster(mesh);
          }
          mesh.dispose();
        }
      });

      // Load the model with detailed error handling
      console.log('Loading model from URL:', modelUrl);
      const result = await SceneLoader.ImportMeshAsync(
        '',
        '',
        modelUrl,
        this.scene,
        (progress) => {
          console.log('Loading progress:', progress);
        }
      );
      
      console.log('Model loaded successfully:', {
        meshes: result.meshes?.length || 0,
        skeletons: result.skeletons?.length || 0,
        animationGroups: result.animationGroups?.length || 0,
        transformNodes: result.transformNodes?.length || 0
      });

      // Check if we have any renderable content
      const renderableMeshes = result.meshes.filter(mesh => 
        mesh && !mesh.isDisposed() && mesh.isEnabled() && mesh.visibility > 0
      );
      
      console.log('Renderable meshes found:', renderableMeshes.length);
      renderableMeshes.forEach((mesh, index) => {
        console.log(`Mesh ${index}:`, {
          name: mesh.name,
          vertices: mesh.getTotalVertices(),
          visible: mesh.visibility,
          enabled: mesh.isEnabled(),
          hasGeometry: !!mesh.geometry,
          hasMaterial: !!mesh.material,
          hasSkeleton: !!mesh.skeleton
        });
      });
      
      if (result.meshes.length > 0 || result.transformNodes.length > 0) {
        // Stop all animations immediately
        if (result.animationGroups && result.animationGroups.length > 0) {
          result.animationGroups.forEach(animGroup => {
            animGroup.stop();
            animGroup.reset();
            console.log(`Stopped animation: ${animGroup.name}`);
          });
        }
        
        // Handle skeletons for proper mesh display
        if (result.skeletons && result.skeletons.length > 0) {
          result.skeletons.forEach(skeleton => {
            // Stop all skeleton animations
            this.scene.stopAnimation(skeleton);
            
            // Reset skeleton to bind pose if possible
            if (skeleton.bones && skeleton.bones.length > 0) {
              skeleton.bones.forEach(bone => {
                // Stop bone animations
                this.scene.stopAnimation(bone);
                
                // Reset bone transformations to rest pose
                if (bone.getRestPose) {
                  try {
                    const restPose = bone.getRestPose();
                    if (restPose.position) bone.setAbsolutePosition(restPose.position);
                    if (restPose.rotationQuaternion) bone.setRotationQuaternion(restPose.rotationQuaternion);
                    if (restPose.scaling) bone.setScale(restPose.scaling);
                  } catch (e) {
                    // Fallback: just ensure bone is in a reasonable state
                    bone.returnToRest();
                  }
                } else if (bone.returnToRest) {
                  bone.returnToRest();
                }
              });
            }
            
            // Ensure skeleton is ready for rendering
            skeleton.prepare();
          });
        }
        
        // Ensure all meshes are visible and properly positioned
        result.meshes.forEach(mesh => {
          // Make sure mesh is visible
          mesh.visibility = 1.0;
          mesh.setEnabled(true);
          
          if (mesh.skeleton) {
            mesh.refreshBoundingInfo();
            mesh.computeWorldMatrix(true);
          }
          
          // Force refresh bounding info
          mesh.refreshBoundingInfo();
        });
        // Calculate bounding box and center the model
        let min = null;
        let max = null;

        result.meshes.forEach((mesh, index) => {
          if (mesh.getBoundingInfo && mesh.getTotalVertices() > 0) {
            mesh.computeWorldMatrix(true);
            mesh.refreshBoundingInfo();
            const boundingInfo = mesh.getBoundingInfo();
            const meshMin = boundingInfo.boundingBox.minimumWorld;
            const meshMax = boundingInfo.boundingBox.maximumWorld;
            
            console.log(`Mesh ${index} bounding box:`, {
              name: mesh.name,
              min: meshMin.toString(),
              max: meshMax.toString(),
              vertices: mesh.getTotalVertices()
            });

            if (!min) {
              min = meshMin.clone();
              max = meshMax.clone();
            } else {
              min = Vector3.Minimize(min, meshMin);
              max = Vector3.Maximize(max, meshMax);
            }
          }
        });
        
        console.log('Overall bounding box:', {
          min: min?.toString(),
          max: max?.toString(),
          hasValidBounds: !!(min && max)
        });

        if (min && max) {
          // Find the largest mesh by volume/surface area
          let largestMesh = null;
          let largestVolume = 0;
          
          result.meshes.forEach(mesh => {
            if (mesh.getBoundingInfo && mesh.getVerticesData) {
              const boundingInfo = mesh.getBoundingInfo();
              const meshSize = boundingInfo.boundingBox.maximumWorld.subtract(boundingInfo.boundingBox.minimumWorld);
              const volume = meshSize.x * meshSize.y * meshSize.z;
              
              if (volume > largestVolume) {
                largestVolume = volume;
                largestMesh = mesh;
              }
            }
          });
          
          // Use largest mesh as focus point, or fallback to overall center
          let focusMesh = largestMesh || result.meshes[0];
          let focusMin, focusMax;
          
          if (focusMesh && focusMesh.getBoundingInfo) {
            const boundingInfo = focusMesh.getBoundingInfo();
            focusMin = boundingInfo.boundingBox.minimumWorld;
            focusMax = boundingInfo.boundingBox.maximumWorld;
          } else {
            focusMin = min;
            focusMax = max;
          }
          
          // Calculate center of the largest part
          const focusCenter = Vector3.Center(focusMin, focusMax);
          const size = max.subtract(min);
          const maxDimension = Math.max(size.x, size.y, size.z);
          
          // Move all meshes so the largest part is centered at origin and on ground
          const groundOffset = min.y; // How much to lift to place bottom on ground
          
          result.meshes.forEach(mesh => {
            if (mesh.position) {
              // Center based on the largest mesh's center
              mesh.position.x -= focusCenter.x;
              mesh.position.z -= focusCenter.z;
              mesh.position.y -= groundOffset; // Place bottom of model at Y=0
            }
            
            // Keep original materials
            // Add mesh to shadow generator
            if (mesh.material) {
              this.shadowGenerator.addShadowCaster(mesh);
              mesh.receiveShadows = true;
            }
          });

          // Recalculate the largest mesh position after repositioning
          if (focusMesh) {
            focusMesh.computeWorldMatrix(true);
            if (focusMesh.getBoundingInfo) {
              focusMesh.getBoundingInfo().update(focusMesh._worldMatrix);
              const newBoundingInfo = focusMesh.getBoundingInfo();
              const newFocusMin = newBoundingInfo.boundingBox.minimumWorld;
              const newFocusMax = newBoundingInfo.boundingBox.maximumWorld;
              const newFocusCenter = Vector3.Center(newFocusMin, newFocusMax);
              
              // Target the exact center of the largest mesh part
              this.camera.target = newFocusCenter;
            } else {
              this.camera.target = Vector3.Zero();
            }
          } else {
            this.camera.target = Vector3.Zero();
          }
          
          // Improved zoom logic - larger objects get progressively closer
          let cameraDistanceMultiplier = 1.0;
          
          if (maxDimension < 0.5) {
            // Very tiny objects
            cameraDistanceMultiplier = 1.8;
          } else if (maxDimension < 2) {
            // Small objects
            cameraDistanceMultiplier = 1.4;
          } else if (maxDimension < 10) {
            // Medium objects
            cameraDistanceMultiplier = 1.2;
          } else if (maxDimension < 50) {
            // Large objects - zoom in more
            cameraDistanceMultiplier = 0.9;
          } else {
            // Huge objects - zoom in even more
            cameraDistanceMultiplier = 0.7;
          }
          
          // Set camera distance to fill frame appropriately
          this.camera.radius = maxDimension * cameraDistanceMultiplier;
          
          this.camera.alpha = Math.PI * 0.75; // 135 degrees - front-right view
          this.camera.beta = Math.PI / 2.5; // 72 degrees - slightly more elevated (panned down)

          // Adjust grid scale based on object size for better visual reference
          const gridScale = Math.max(1, maxDimension / 10);
          this.ground.scaling = new Vector3(gridScale, 1, gridScale);
          
          // Adjust grid frequency for different scales
          const gridMat = this.ground.material;
          if (maxDimension < 2) {
            gridMat.majorUnitFrequency = 10;
            gridMat.gridRatio = 0.5;
          } else if (maxDimension < 10) {
            gridMat.majorUnitFrequency = 5;
            gridMat.gridRatio = 1;
          } else {
            gridMat.majorUnitFrequency = 2;
            gridMat.gridRatio = 2;
          }

          // Adjust light position based on model size
          this.dirLight.position = new Vector3(
            maxDimension * 2,
            maxDimension * 3,
            maxDimension * 2
          );
        }

        // Render the scene multiple times for better quality
        this.scene.render();
        this.scene.render(); // Second render for shadows to settle

        // Capture the canvas as base64
        const thumbnailDataUrl = this.canvas.toDataURL('image/png', 0.95);
        
        // Cache the result
        this.thumbnailCache.set(cacheKey, thumbnailDataUrl);

        // Clean up loaded meshes
        result.meshes.forEach(mesh => {
          if (mesh && !mesh.isDisposed()) {
            if (this.shadowGenerator.getShadowMap().renderList.includes(mesh)) {
              this.shadowGenerator.removeShadowCaster(mesh);
            }
            mesh.dispose();
          }
        });

        return thumbnailDataUrl;
      }
    } catch (error) {
      console.error('Error generating model thumbnail:', error);
      return null;
    }
  }

  dispose() {
    if (this.shadowGenerator) {
      this.shadowGenerator.dispose();
    }
    if (this.ground) {
      this.ground.dispose();
    }
    // Sky elements will be disposed with the scene
    if (this.scene) {
      this.scene.dispose();
    }
    if (this.engine) {
      this.engine.dispose();
    }
    if (this.canvas && this.canvas.parentNode) {
      this.canvas.parentNode.removeChild(this.canvas);
    }
    this.thumbnailCache.clear();
    this.isInitialized = false;
  }

  clearCache() {
    this.thumbnailCache.clear();
  }

  getCachedThumbnail(modelPath) {
    return this.thumbnailCache.get(modelPath);
  }
}

// Create a singleton instance
export const modelThumbnailGenerator = new ModelThumbnailGenerator();