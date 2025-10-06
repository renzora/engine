import { createSignal, createEffect, onMount, onCleanup, Show, For, createMemo } from 'solid-js';
import { renderStore } from '@/render/store';
import { editorActions } from '@/layout/stores/EditorStore.jsx';
import ContextMenu from '@/ui/ContextMenu.jsx';
import { 
  IconPalette, 
  IconSphere,
  IconBox,
  IconCube,
  IconSettings,
  IconPhoto,
  IconCircleDot,
  IconMinus,
  IconPlus,
  IconX,
  IconClock,
  IconHash,
  IconWave,
  IconMath,
  IconMathFunction,
  IconVector,
  IconColorFilter,
  IconAdjustments,
  IconGradient,
  IconTexture,
  IconSquare,
  IconCircle,
  IconHexagon,
  IconBulb,
  IconVideo,
  IconWorld
} from '@tabler/icons-solidjs';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial.js';
import { PBRMaterial } from '@babylonjs/core/Materials/PBR/pbrMaterial.js';
import { NodeMaterial } from '@babylonjs/core/Materials/Node/nodeMaterial.js';
import { Color3 } from '@babylonjs/core/Maths/math.color.js';
import { bridgeService } from '@/plugins/core/bridge';
import { bottomPanelVisible, propertiesPanelVisible } from '@/api/plugin';

// Node Material Blocks
import { InputBlock } from '@babylonjs/core/Materials/Node/Blocks/Input/inputBlock.js';
import { FragmentOutputBlock } from '@babylonjs/core/Materials/Node/Blocks/Fragment/fragmentOutputBlock.js';
import { TextureBlock } from '@babylonjs/core/Materials/Node/Blocks/Dual/textureBlock.js';
import { MultiplyBlock } from '@babylonjs/core/Materials/Node/Blocks/multiplyBlock.js';
import { AddBlock } from '@babylonjs/core/Materials/Node/Blocks/addBlock.js';
import { LerpBlock } from '@babylonjs/core/Materials/Node/Blocks/lerpBlock.js';
import { FresnelBlock } from '@babylonjs/core/Materials/Node/Blocks/fresnelBlock.js';
import { ClampBlock } from '@babylonjs/core/Materials/Node/Blocks/clampBlock.js';
// Removing PowBlock for now - will implement later
import { CreateSphere } from '@babylonjs/core/Meshes/Builders/sphereBuilder.js';
import { CreateBox } from '@babylonjs/core/Meshes/Builders/boxBuilder.js';
import { CreateGround } from '@babylonjs/core/Meshes/Builders/groundBuilder.js';
import { CreateCylinder } from '@babylonjs/core/Meshes/Builders/cylinderBuilder.js';
import { CreateTorus } from '@babylonjs/core/Meshes/Builders/torusBuilder.js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';
import { FreeCamera } from '@babylonjs/core/Cameras/freeCamera.js';
import { ArcRotateCamera } from '@babylonjs/core/Cameras/arcRotateCamera.js';
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight.js';
import { DirectionalLight } from '@babylonjs/core/Lights/directionalLight.js';
import { ShadowGenerator } from '@babylonjs/core/Lights/Shadows/shadowGenerator.js';
import { Scene } from '@babylonjs/core/scene.js';
import { Engine } from '@babylonjs/core/Engines/engine.js';
import { Texture } from '@babylonjs/core/Materials/Textures/texture.js';
import { HDRCubeTexture } from '@babylonjs/core/Materials/Textures/hdrCubeTexture.js';
import { CubeTexture } from '@babylonjs/core/Materials/Textures/cubeTexture.js';
// Import EXR loader for HDR textures
import '@babylonjs/loaders/glTF';
import '@babylonjs/core/Materials/Textures/Loaders/exrTextureLoader.js';

// Texture Preview Component - Prevents image reloading during drag
function TexturePreview(props) {
  const imageSrc = createMemo(() => {
    const asset = props.asset;
    if (!asset) return '';
    
    // Try different thumbnail URL formats based on what we see in the console
    if (asset.thumbnailUrl) {
      return asset.thumbnailUrl;
    } else if (asset.path) {
      // Construct thumbnail URL based on the path pattern we see in console logs
      const pathWithoutExt = asset.path.replace(/\.[^/.]+$/, ""); // Remove extension
      const cleanPath = pathWithoutExt.replace(/[\/\\]/g, '_'); // Replace slashes with underscores
      return `http://localhost:3001/file/projects/test/.cache/thumbnails/${cleanPath}_${asset.extension}_256.png`;
    } else if (asset.name) {
      // Fallback using name
      const nameWithoutExt = asset.name.replace(/\.[^/.]+$/, "");
      const cleanName = nameWithoutExt.replace(/[\/\\]/g, '_');
      return `http://localhost:3001/file/projects/test/.cache/thumbnails/${cleanName}_${asset.extension || 'jpg'}_256.png`;
    }
    
    // Last resort - try the original API format
    return `/api/assets/thumbnail/${asset.id}`;
  });
  const imageAlt = createMemo(() => props.asset?.name || 'Texture');
  
  return (
    <div class="relative overflow-hidden rounded border border-base-300 bg-base-200 h-16 mb-2">
      <img 
        src={imageSrc()}
        alt={imageAlt()}
        class="w-full h-full object-cover"
        onLoad={() => {
          console.log('✅ Texture preview loaded:', imageSrc());
        }}
        onError={(e) => {
          console.log('❌ Texture preview failed to load:', imageSrc());
          console.log('Asset data:', props.asset);
          e.target.style.display = 'none';
          e.target.nextElementSibling.style.display = 'flex';
        }}
      />
      <div class="absolute inset-0 bg-base-300 flex items-center justify-center text-base-content/40 text-xs hidden">
        No Preview
      </div>
      <div class="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/80 to-transparent px-1 py-0.5">
        <div class="text-white text-xs truncate">{imageAlt()}</div>
      </div>
    </div>
  );
}

// Connection Line Component - Reactive to node drag transforms
function ConnectionLine(props) {
  const pathData = createMemo(() => {
    const { connection, nodes, getSocketScreenPosition, zoom, pan, draggedNodeId, draggedNodeTransform } = props;
    
    // Find the actual nodes to ensure we have current positions
    const fromNode = nodes.find(n => n.id === connection.from.nodeId);
    const toNode = nodes.find(n => n.id === connection.to.nodeId);
    
    if (!fromNode || !toNode) return '';
    
    // Access signals to make this reactive
    const currentZoom = zoom();
    const currentPan = pan();
    const currentDragTransform = draggedNodeTransform(); // Make reactive to drag transform
    
    // Check if this connection involves the dragged node
    const isDraggedConnection = draggedNodeId === connection.from.nodeId || draggedNodeId === connection.to.nodeId;
    
    // Get socket positions in screen coordinates
    const fromPos = getSocketScreenPosition(connection.from.nodeId, connection.from.socketId, 'output');
    const toPos = getSocketScreenPosition(connection.to.nodeId, connection.to.socketId, 'input');
    
    if (!fromPos || !toPos) return '';
    
    const fromX = fromPos.x;
    const fromY = fromPos.y;
    const toX = toPos.x;
    const toY = toPos.y;
    
    // Create curved connection with better control points
    const controlOffset = Math.max(80, Math.abs(toX - fromX) * 0.4);
    return `M ${fromX} ${fromY} C ${fromX + controlOffset} ${fromY} ${toX - controlOffset} ${toY} ${toX} ${toY}`;
  });
  
  return (
    <g>
      {/* Invisible thicker path for easier clicking */}
      <path
        d={pathData()}
        stroke="transparent"
        stroke-width="12"
        fill="none"
        class="cursor-pointer"
        style={{'pointer-events': 'all'}}
        onClick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          console.log('Connection clicked, removing:', props.connection.id);
          props.onRemove?.(props.connection.id);
        }}
      />
      {/* Visible connection line */}
      <path
        d={pathData()}
        stroke="#3b82f6"
        stroke-width="3"
        fill="none"
        class="drop-shadow-sm pointer-events-none"
      />
    </g>
  );
}

export default function MaterialsViewport() {
  const [previewShape, setPreviewShape] = createSignal('sphere');
  const [nodes, setNodes] = createSignal([]);
  const [connections, setConnections] = createSignal([]);
  const [draggedNode, setDraggedNode] = createSignal(null);
  const [dragOffset, setDragOffset] = createSignal({ x: 0, y: 0 });
  const [selectedNode, setSelectedNode] = createSignal(null);
  const [draggedNodeTransform, setDraggedNodeTransform] = createSignal({ x: 0, y: 0 });
  const [maxZIndex, setMaxZIndex] = createSignal(1);
  const [nodeZIndices, setNodeZIndices] = createSignal(new Map());
  const [currentMaterial, setCurrentMaterial] = createSignal(null);
  const [zoom, setZoom] = createSignal(1);
  const [pan, setPan] = createSignal({ x: 0, y: 0 });
  const [isPanning, setIsPanning] = createSignal(false);
  const [panStart, setPanStart] = createSignal({ x: 0, y: 0 });
  const [draggingConnection, setDraggingConnection] = createSignal(null);
  const [dragConnectionEnd, setDragConnectionEnd] = createSignal({ x: 0, y: 0 });
  const [hoveredSocket, setHoveredSocket] = createSignal(null);
  const [isDraggingAllNodes, setIsDraggingAllNodes] = createSignal(false);
  const [allNodesDragStart, setAllNodesDragStart] = createSignal({ x: 0, y: 0 });
  const [contextMenu, setContextMenu] = createSignal(null);
  const [contextMenuPosition, setContextMenuPosition] = createSignal(null);
  
  // Preview camera controls
  const [cameraDistance, setCameraDistance] = createSignal(6);
  const [isRotatingCamera, setIsRotatingCamera] = createSignal(false);
  const [lastMousePos, setLastMousePos] = createSignal({ x: 0, y: 0 });
  
  // Environment and lighting controls
  const [lightIntensity, setLightIntensity] = createSignal(0.8);
  const [ambientIntensity, setAmbientIntensity] = createSignal(0.4);
  const [shadowsEnabled, setShadowsEnabled] = createSignal(true);
  const [shadowQuality, setShadowQuality] = createSignal(1024);
  const [backgroundType, setBackgroundType] = createSignal('color'); // 'color', 'hdr'
  const [backgroundColor, setBackgroundColor] = createSignal('#262626');
  const [hdrBackground, setHdrBackground] = createSignal(null); // Asset for HDR background
  const [usePBR, setUsePBR] = createSignal(false);
  
  // Additional preview settings
  const [wireframeMode, setWireframeMode] = createSignal(false);
  const [autoRotate, setAutoRotate] = createSignal(false);
  const [groundVisible, setGroundVisible] = createSignal(true);
  const [meshScale, setMeshScale] = createSignal(1.0);
  const [unlitMode, setUnlitMode] = createSignal(false);
  
  // Section collapse state
  const [sectionsOpen, setSectionsOpen] = createSignal({
    material: true,
    camera: true,
    lighting: true,
    environment: true,
    preview: true
  });
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };
  
  // Throttle mouse move updates for better performance
  let lastMoveTime = 0;
  const MOVE_THROTTLE_MS = 16; // ~60fps
  
  // Preview scene refs
  let previewCanvasRef;
  let previewScene;
  let previewEngine;
  let previewMesh;
  let groundMesh;
  let backdropMesh;
  let shadowGenerator;
  let nodeGraphRef;
  let previewCamera;
  let directionalLight;
  let ambientLight;
  
  // Update camera distance for zoom
  const updateCameraDistance = () => {
    if (!previewCamera) return;
    previewCamera.radius = cameraDistance();
  };

  // Update lighting
  const updateLighting = () => {
    if (directionalLight) {
      directionalLight.intensity = lightIntensity();
    }
    if (ambientLight) {
      ambientLight.intensity = ambientIntensity();
    }
  };

  // Update shadows
  const updateShadows = () => {
    if (!shadowGenerator || !previewMesh || !groundMesh) return;
    
    if (shadowsEnabled()) {
      shadowGenerator.addShadowCaster(previewMesh);
      groundMesh.receiveShadows = true;
    } else {
      shadowGenerator.removeShadowCaster(previewMesh);
      groundMesh.receiveShadows = false;
    }
  };

  // Update wireframe mode
  const updateWireframe = () => {
    if (!previewMesh) return;
    
    if (previewMesh.material) {
      previewMesh.material.wireframe = wireframeMode();
    }
  };

  // Update ground visibility
  const updateGroundVisibility = () => {
    if (!groundMesh) return;
    groundMesh.setEnabled(groundVisible());
  };

  // Update mesh scale
  const updateMeshScale = () => {
    if (!previewMesh) return;
    const scale = meshScale();
    previewMesh.scaling = new Vector3(scale, scale, scale);
  };

  // Reset camera to default position
  const resetCamera = () => {
    if (!previewCamera) return;
    previewCamera.setTarget(new Vector3(0, -0.5, 0));
    previewCamera.alpha = Math.PI / 4;
    previewCamera.beta = Math.PI / 3;
    previewCamera.radius = 6;
  };

  // Auto rotation logic
  let autoRotationAnimation;
  const updateAutoRotation = () => {
    if (!previewMesh) return;
    
    if (autoRotate()) {
      // Start auto rotation
      let rotationSpeed = 0.005;
      autoRotationAnimation = setInterval(() => {
        if (previewMesh && autoRotate()) {
          previewMesh.rotation.y += rotationSpeed;
        }
      }, 16); // ~60fps
    } else {
      // Stop auto rotation
      if (autoRotationAnimation) {
        clearInterval(autoRotationAnimation);
        autoRotationAnimation = null;
      }
    }
  };

  // Update background
  const updateBackground = () => {
    if (!previewScene) return;
    
    if (backgroundType() === 'color') {
      const hexColor = backgroundColor();
      const r = parseInt(hexColor.slice(1, 3), 16) / 255;
      const g = parseInt(hexColor.slice(3, 5), 16) / 255;
      const b = parseInt(hexColor.slice(5, 7), 16) / 255;
      previewScene.clearColor = new Color3(r, g, b);
      
      // Clear HDR environment when switching to color
      clearHDREnvironment();
      
      // Create a simple environment texture from the color for PBR reflections
      createColorEnvironmentTexture(hexColor);
      
    } else if (backgroundType() === 'hdr' && hdrBackground()) {
      const asset = hdrBackground();
      console.log('updateBackground: HDR mode with asset:', asset);
      
      // Handle uploaded files
      if (asset.isUploaded && asset.file) {
        console.log('Loading uploaded HDR file:', asset.file.name);
        loadUploadedHDRFile(asset.file);
        return;
      }
      
      // Handle assets from project (original logic)
      const hdrUrl = constructTextureUrl(asset);
      
      if (hdrUrl) {
        console.log('Loading HDR environment:', hdrUrl);
        
        // Debug: Try multiple URL variations
        const debugUrls = [
          hdrUrl,
          hdrUrl.replace('/assets/materials/', '/assets/'),
          hdrUrl.replace('/assets/materials/', '/'),
          hdrUrl.replace('.hdr', '.HDR'),
          // Try the bridge API format
          `http://localhost:3001/api/file/${asset.path}`,
          `http://localhost:3001/api/assets/${asset.name}`,
          // Try without file prefix
          `http://localhost:3001/projects/test/${asset.path}`,
        ];
        
        console.log('Testing multiple URL variations:', debugUrls);
        
        // Test each URL
        const testUrls = debugUrls.map(url => 
          fetch(url, { method: 'HEAD' })
            .then(response => ({ url, status: response.status, ok: response.ok }))
            .catch(error => ({ url, status: 'ERROR', error }))
        );
        
        Promise.all(testUrls).then(results => {
          console.log('URL test results:', results);
          
          const workingUrl = results.find(result => result.ok);
          if (workingUrl) {
            console.log('✅ Found working URL:', workingUrl.url);
            loadHDRTexture(workingUrl.url);
          } else {
            console.error('❌ No working URLs found. Trying bridge service...');
            tryBridgeService(asset);
          }
        });
      } else {
        console.error('❌ Could not construct HDR URL from asset');
      }
    }
  };

  // Separate function to load HDR texture
  const loadHDRTexture = (hdrUrl) => {
    try {
      console.log('Attempting HDR loading with multiple methods...');
      
      // Add a timeout to detect silent failures
      let loadTimeout = setTimeout(() => {
        console.warn('⚠️ HDR loading timeout - trying fallback methods');
        tryRegularTexture(hdrUrl);
      }, 5000); // 5 second timeout
      
      // Method 1: Try HDRCubeTexture
      console.log('Method 1: Trying HDRCubeTexture...');
      const hdrTexture = new HDRCubeTexture(hdrUrl, previewScene, 256); // Increased size
      
      hdrTexture.onLoad = () => {
        console.log('✅ HDR environment loaded successfully via HDRCubeTexture');
        clearTimeout(loadTimeout);
        
        // Set the environment texture
        previewScene.environmentTexture = hdrTexture;
        
        // Create skybox
        previewScene.createDefaultSkybox(hdrTexture, true, 1000);
        
        // Set environment intensity
        previewScene.environmentIntensity = 1.2;
        
        console.log('Environment texture set:', previewScene.environmentTexture);
        console.log('Scene background:', previewScene.clearColor);
      };
      
      hdrTexture.onError = (error) => {
        console.error('❌ HDRCubeTexture failed:', error);
        clearTimeout(loadTimeout);
        
        // Method 2: Try regular texture with environment mapping
        console.log('Method 2: Trying regular Texture with environment mapping...');
        tryEnvironmentTexture(hdrUrl);
      };
      
      // Log the texture object for debugging
      console.log('HDRCubeTexture object created:', hdrTexture);
      
    } catch (loadError) {
      console.error('❌ Exception during HDR loading:', loadError);
      tryEnvironmentTexture(hdrUrl);
    }
  };

  // Try environment texture method
  const tryEnvironmentTexture = (hdrUrl) => {
    console.log('Method 2: Trying regular Texture with environment setup...');
    const envTexture = new Texture(hdrUrl, previewScene);
    
    envTexture.onLoad = () => {
      console.log('✅ HDR loaded as environment texture');
      
      // Set as environment texture
      previewScene.environmentTexture = envTexture;
      previewScene.environmentIntensity = 1.5;
      
      // Create a simple colored background to show something is working
      previewScene.clearColor = new Color3(0.2, 0.4, 0.8); // Blue background
      
      console.log('Environment texture setup complete with blue background');
    };
    
    envTexture.onError = (envError) => {
      console.error('❌ Environment texture failed:', envError);
      // Create a simple environment effect even without the HDR
      createSimpleEnvironment();
    };
  };

  // Create a simple environment effect when HDR fails
  const createSimpleEnvironment = () => {
    console.log('Creating simple environment effect...');
    
    // Change background to a gradient-like color
    previewScene.clearColor = new Color3(0.1, 0.2, 0.4);
    
    // Enhance lighting to simulate environment lighting
    if (directionalLight) {
      directionalLight.intensity = lightIntensity() * 1.5;
    }
    if (ambientLight) {
      ambientLight.intensity = ambientIntensity() * 2.0;
    }
    
    console.log('✅ Simple environment effect applied');
  };

  // Fallback to regular texture
  const tryRegularTexture = (hdrUrl) => {
    console.log('Method 3: Trying regular Texture as final fallback...');
    const fallbackTexture = new Texture(hdrUrl, previewScene);
    
    fallbackTexture.onLoad = () => {
      console.log('✅ HDR loaded as regular texture (fallback)');
      previewScene.environmentTexture = fallbackTexture;
      previewScene.clearColor = new Color3(0.2, 0.4, 0.8); // Blue background to show it worked
      console.log('Final fallback environment texture set');
    };
    
    fallbackTexture.onError = (fallbackError) => {
      console.error('❌ All HDR loading methods failed:', fallbackError);
      console.log('Applying simple environment as last resort...');
      createSimpleEnvironment();
    };
  };

  // Try loading HDR via bridge service
  const tryBridgeService = async (asset) => {
    try {
      console.log('Trying to load HDR via bridge service...');
      console.log('Asset path:', asset.path);
      
      // Try to read the file via bridge service
      const fileData = await bridgeService.readFile(asset.path);
      console.log('✅ Bridge service read file successfully');
      
      // Create a blob URL from the file data
      const blob = new Blob([fileData], { type: 'application/octet-stream' });
      const blobUrl = URL.createObjectURL(blob);
      
      console.log('Created blob URL:', blobUrl);
      
      // Try loading HDR from blob URL
      loadHDRTexture(blobUrl);
      
    } catch (error) {
      console.error('❌ Bridge service failed:', error);
      console.log('Trying HDR loading with original URL anyway...');
      loadHDRTexture(constructTextureUrl(asset)); // Last resort
    }
  };

  // Handle HDR file upload from filesystem
  const handleHDRFileUpload = (event) => {
    const file = event.target.files[0];
    if (!file) return;
    
    console.log('HDR file selected:', file.name, file.type, file.size);
    
    // Check if it's an HDR file
    const isValidHDR = file.name.match(/\.(hdr|exr|dds|ktx)$/i);
    if (!isValidHDR) {
      console.error('Invalid file type. Please select an HDR, EXR, DDS, or KTX file.');
      return;
    }
    
    // Create a fake asset object for the uploaded file
    const uploadedAsset = {
      name: file.name,
      path: null, // No path for uploaded files
      file: file, // Store the actual file object
      isUploaded: true
    };
    
    setHdrBackground(uploadedAsset);
    console.log('HDR background set to uploaded file:', file.name);
    
    // Clear the file input so the same file can be selected again
    event.target.value = '';
  };

  // Load HDR file from uploaded file object (based on Sky panel approach)
  const loadUploadedHDRFile = (file) => {
    console.log('🌍 Loading uploaded HDR file with Sky panel approach:', file.name);
    
    // Create file URL for HDR files (same as Sky panel)
    const fileUrl = URL.createObjectURL(file);
    console.log('🌍 Created file URL:', fileUrl);
    
    // Use the same approach as Sky panel
    loadHDRWithNativeBabylonJS(fileUrl, file.name);
  };

  // Load HDR with native BabylonJS support (copied from Sky panel)
  const loadHDRWithNativeBabylonJS = (hdrUrl, fileName) => {
    console.log('🌍 Loading HDR with native BabylonJS support:', fileName);
    
    let loadHandled = false;
    
    try {
      // Try HDRCubeTexture with proper material setup (same as Sky panel)
      console.log('🌍 Trying HDR loading: HDRCubeTexture with proper setup');
      
      const hdrTexture = new HDRCubeTexture(hdrUrl, previewScene, 512);
      
      console.log('🌍 HDR texture created:', {
        url: hdrUrl,
        isReady: hdrTexture.isReady(),
        hasObservable: !!hdrTexture.onLoadObservable
      });
      
      // Add error handling for load failures
      if (hdrTexture.onErrorObservable) {
        hdrTexture.onErrorObservable.add((error) => {
          console.error('❌ HDR texture load error:', error);
        });
      }
      
      // Test direct URL fetch (same as Sky panel)
      fetch(hdrUrl)
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

      // Load success handler
      hdrTexture.onLoadObservable.addOnce(() => {
        loadHandled = true;
        console.log('🌍 HDR texture loaded via observable, applying to environment...');
        applyHDRToEnvironment(hdrTexture);
      });
      
      // Multiple fallbacks to ensure HDR gets applied (same as Sky panel)
      const checkAndApplyHDR = () => {
        if (!loadHandled && hdrTexture.isReady()) {
          loadHandled = true;
          console.log('🔄 HDR texture ready via polling, applying fallback setup...');
          applyHDRToEnvironment(hdrTexture);
        } else if (!loadHandled) {
          console.log('🌍 HDR texture not ready yet, will retry...');
        }
      };
      
      // Check multiple times in case observable doesn't fire
      setTimeout(checkAndApplyHDR, 100);
      setTimeout(checkAndApplyHDR, 500);
      setTimeout(checkAndApplyHDR, 1000);
      setTimeout(checkAndApplyHDR, 2000);
      
    } catch (error) {
      console.error('❌ HDRCubeTexture approach failed:', error);
    }
    
    // ArrayBuffer fallback approach (adapted from Sky panel)
    setTimeout(() => {
      if (!loadHandled) {
        console.log('🔄 HDR texture failed to load, trying ArrayBuffer approach...');
        
        fetch(hdrUrl)
          .then(response => response.arrayBuffer())
          .then(arrayBuffer => {
            console.log('🌍 HDR ArrayBuffer loaded, size:', arrayBuffer.byteLength);
            
            // Create blob URL with proper MIME type
            const blob = new Blob([arrayBuffer], { type: 'image/vnd.radiance' });
            const blobUrl = URL.createObjectURL(blob);
            
            console.log('🌍 Created blob URL for HDR:', blobUrl);
            
            const hdrFromBlob = new HDRCubeTexture(blobUrl, previewScene, 512);
            hdrFromBlob.onLoadObservable.addOnce(() => {
              loadHandled = true;
              console.log('🌍 HDR from blob loaded successfully!');
              applyHDRToEnvironment(hdrFromBlob);
              
              // Clean up blob URL
              URL.revokeObjectURL(blobUrl);
            });
            
            // Final fallback to regular texture
            setTimeout(() => {
              if (!loadHandled) {
                console.log('🔄 Blob approach failed, trying regular texture...');
                try {
                  const fallbackTexture = new Texture(hdrUrl, previewScene);
                  fallbackTexture.onLoadObservable.addOnce(() => {
                    console.log('🌍 Regular texture loaded, applying as environment...');
                    loadHandled = true;
                    applyRegularTextureAsEnvironment(fallbackTexture);
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
  };

  // Apply HDR texture to environment (adapted from Sky panel)
  const applyHDRToEnvironment = (hdrTexture) => {
    console.log('🌍 Applying HDR texture to environment...');
    console.log('🌍 HDR texture details:', {
      isReady: hdrTexture.isReady(),
      size: hdrTexture.getSize ? hdrTexture.getSize() : 'unknown',
      url: hdrTexture.url
    });
    
    // Set environment texture for PBR lighting
    previewScene.environmentTexture = hdrTexture;
    previewScene.environmentIntensity = 1.2;
    
    // Create skybox for background
    previewScene.createDefaultSkybox(hdrTexture, true, 1000);
    
    console.log('✅ HDR texture applied to environment');
  };

  // Apply regular texture as environment fallback
  const applyRegularTextureAsEnvironment = (texture) => {
    console.log('🌍 Applying regular texture as environment fallback...');
    
    previewScene.environmentTexture = texture;
    previewScene.environmentIntensity = 1.0;
    
    // Change background to show something worked
    previewScene.clearColor = new Color3(0.2, 0.4, 0.8);
    
    console.log('✅ Regular texture applied as environment');
  };

  // Create a simple environment texture from color for PBR reflections
  const createColorEnvironmentTexture = (hexColor) => {
    console.log('🎨 Creating color environment texture for PBR:', hexColor);
    
    try {
      // Create a simple canvas texture with the color
      const canvas = document.createElement('canvas');
      canvas.width = 256;
      canvas.height = 256;
      const ctx = canvas.getContext('2d');
      
      // Fill with the background color
      ctx.fillStyle = hexColor;
      ctx.fillRect(0, 0, 256, 256);
      
      // Create texture from canvas
      const colorTexture = new Texture('data:' + canvas.toDataURL(), previewScene);
      
      // Set as environment texture for PBR reflections
      colorTexture.coordinatesMode = Texture.SKYBOX_MODE;
      previewScene.environmentTexture = colorTexture;
      previewScene.environmentIntensity = 0.8; // Subtle environment effect
      
      console.log('✅ Color environment texture created for PBR reflections');
      
    } catch (error) {
      console.error('❌ Failed to create color environment texture:', error);
    }
  };

  // Clear HDR environment when switching back to color mode
  const clearHDREnvironment = () => {
    console.log('🗑️ Clearing HDR environment...');
    
    if (previewScene.environmentTexture) {
      // Dispose of the environment texture
      previewScene.environmentTexture.dispose();
      previewScene.environmentTexture = null;
      console.log('🗑️ Environment texture disposed');
    }
    
    // Reset environment intensity
    previewScene.environmentIntensity = 1.0;
    
    // Find and remove ALL potential skybox meshes (more comprehensive search)
    const allMeshes = previewScene.meshes.slice(); // Create a copy to avoid modification during iteration
    let skyboxesRemoved = 0;
    
    allMeshes.forEach(mesh => {
      const meshName = mesh.name.toLowerCase();
      // Check for various skybox naming patterns
      if (meshName.includes('skybox') || 
          meshName.includes('sky') || 
          meshName.includes('environment') ||
          meshName.includes('envbox') ||
          (mesh.material && mesh.material.reflectionTexture && 
           mesh.material.reflectionTexture.coordinatesMode === Texture.SKYBOX_MODE)) {
        
        console.log('🗑️ Found potential skybox mesh:', mesh.name, mesh.material?.constructor.name);
        mesh.dispose();
        skyboxesRemoved++;
      }
    });
    
    console.log('🗑️ Removed', skyboxesRemoved, 'skybox meshes');
    
    // Also clear the default skybox if it exists (Babylon.js creates these)
    if (previewScene._defaultSkybox) {
      previewScene._defaultSkybox.dispose();
      previewScene._defaultSkybox = null;
      console.log('🗑️ Default skybox disposed');
    }
    
    // Force clear any background meshes
    if (previewScene._backgroundSkybox) {
      previewScene._backgroundSkybox.dispose();
      previewScene._backgroundSkybox = null;
      console.log('🗑️ Background skybox disposed');
    }
    
    // Clear any HDR background asset
    if (hdrBackground()) {
      const currentHDR = hdrBackground();
      // Clean up blob URL if it was created from file input
      if (currentHDR && typeof currentHDR === 'object' && currentHDR.isUploaded && currentHDR.file) {
        const fileUrl = URL.createObjectURL(currentHDR.file);
        URL.revokeObjectURL(fileUrl);
        console.log('🗑️ Cleaned up blob URL for uploaded HDR');
      }
      setHdrBackground(null);
    }
    
    // Force scene to re-render
    previewScene.markAllMaterialsAsDirty();
    
    console.log('✅ HDR environment cleared completely');
    console.log('🔍 Remaining meshes:', previewScene.meshes.map(m => m.name));
  };

  // Node types
  const NODE_TYPES = {
    // Output Blocks
    MATERIAL_OUTPUT: 'MaterialOutput',
    VERTEX_OUTPUT: 'VertexOutput',
    FRAGMENT_OUTPUT: 'FragmentOutput',
    DISCARD: 'Discard',
    
    // Input Blocks
    FLOAT: 'Float',
    VECTOR2: 'Vector2',
    VECTOR3: 'Vector3',
    VECTOR4: 'Vector4',
    COLOR3: 'Color3',
    COLOR4: 'Color4',
    TEXTURE: 'Texture',
    REFLECTION_TEXTURE: 'ReflectionTexture',
    IMAGE_SOURCE: 'ImageSource',
    TIME: 'Time',
    DELTA_TIME: 'DeltaTime',
    SCREEN_SIZE: 'ScreenSize',
    FRAG_COORD: 'FragCoord',
    MATERIAL_ALPHA: 'MaterialAlpha',
    CAMERA_POSITION: 'CameraPosition',
    
    // Mesh Inputs
    MESH_POSITION: 'MeshPosition',
    MESH_NORMAL: 'MeshNormal',
    MESH_TANGENT: 'MeshTangent',
    MESH_UV: 'MeshUV',
    MESH_COLOR: 'MeshColor',
    WORLD_POSITION: 'WorldPosition',
    WORLD_NORMAL: 'WorldNormal',
    WORLD_TANGENT: 'WorldTangent',
    
    // Transform Blocks
    TRANSFORM: 'Transform',
    BONES: 'Bones',
    MORPH_TARGETS: 'MorphTargets',
    INSTANCE_COLOR: 'InstanceColor',
    
    // Math Blocks - Basic
    ADD: 'Add',
    SUBTRACT: 'Subtract',
    MULTIPLY: 'Multiply',
    DIVIDE: 'Divide',
    POWER: 'Power',
    SCALE: 'Scale',
    NEGATE: 'Negate',
    ONE_MINUS: 'OneMinus',
    RECIPROCAL: 'Reciprocal',
    
    // Math Blocks - Scientific
    ABS: 'Abs',
    SIGN: 'Sign',
    FLOOR: 'Floor',
    CEILING: 'Ceiling',
    ROUND: 'Round',
    FRACT: 'Fract',
    MOD: 'Mod',
    MIN: 'Min',
    MAX: 'Max',
    SQRT: 'Sqrt',
    EXP: 'Exp',
    EXP2: 'Exp2',
    LOG: 'Log',
    LOG2: 'Log2',
    POW: 'Pow',
    
    // Trigonometry
    SIN: 'Sin',
    COS: 'Cos',
    TAN: 'Tan',
    ASIN: 'ASin',
    ACOS: 'ACos',
    ATAN: 'ATan',
    ATAN2: 'ATan2',
    RADIANS: 'Radians',
    DEGREES: 'Degrees',
    
    // Vector Math
    DOT: 'Dot',
    CROSS: 'Cross',
    NORMALIZE: 'Normalize',
    LENGTH: 'Length',
    DISTANCE: 'Distance',
    REFLECT: 'Reflect',
    REFRACT: 'Refract',
    FRESNEL: 'Fresnel',
    DERIVATIVE: 'Derivative',
    
    // Range/Interpolation
    CLAMP: 'Clamp',
    SATURATE: 'Saturate',
    REMAP: 'Remap',
    LERP: 'Lerp',
    NLERP: 'NLerp',
    STEP: 'Step',
    SMOOTHSTEP: 'SmoothStep',
    
    // Logical
    AND: 'And',
    OR: 'Or',
    XOR: 'Xor',
    NOT: 'Not',
    EQUAL: 'Equal',
    NOT_EQUAL: 'NotEqual',
    GREATER_THAN: 'GreaterThan',
    GREATER_OR_EQUAL: 'GreaterOrEqual',
    LESS_THAN: 'LessThan',
    LESS_OR_EQUAL: 'LessOrEqual',
    
    // Conversion
    COLOR_MERGER: 'ColorMerger',
    COLOR_SPLITTER: 'ColorSplitter',
    VECTOR_MERGER: 'VectorMerger',
    VECTOR_SPLITTER: 'VectorSplitter',
    
    // Color Management
    DESATURATE: 'Desaturate',
    GRADIENT: 'Gradient',
    POSTERIZE: 'Posterize',
    REPLACE_COLOR: 'ReplaceColor',
    
    // Matrices
    WORLD_MATRIX: 'WorldMatrix',
    WORLD_VIEW_MATRIX: 'WorldViewMatrix',
    WORLD_VIEW_PROJECTION_MATRIX: 'WorldViewProjectionMatrix',
    VIEW_MATRIX: 'ViewMatrix',
    VIEW_PROJECTION_MATRIX: 'ViewProjectionMatrix',
    PROJECTION_MATRIX: 'ProjectionMatrix',
    MATRIX_BUILDER: 'MatrixBuilder',
    
    // Noise
    SIMPLEX_PERLIN_3D: 'SimplexPerlin3D',
    VORONOI_NOISE: 'VoronoiNoise',
    WORLEY_NOISE_3D: 'WorleyNoise3D',
    CLOUD: 'Cloud',
    RANDOM_NUMBER: 'RandomNumber',
    
    // Texture Operations
    TRIPLANAR: 'TriPlanar',
    BIPLANAR: 'BiPlanar',
    NORMAL_MAP: 'NormalMap',
    NORMAL_BLEND: 'NormalBlend',
    PARALLAX: 'Parallax',
    TWIRL: 'Twirl',
    
    // Particle
    PARTICLE_COLOR: 'ParticleColor',
    PARTICLE_TEXTURE: 'ParticleTexture',
    PARTICLE_UV: 'ParticleUV',
    PARTICLE_POSITION: 'ParticlePosition',
    PARTICLE_RAMP_GRADIENT: 'ParticleRampGradient',
    
    // PBR
    PBR_METALLIC_ROUGHNESS: 'PBRMetallicRoughness',
    ANISOTROPY: 'Anisotropy',
    CLEARCOAT: 'ClearCoat',
    IRIDESCENCE: 'Iridescence',
    SHEEN: 'Sheen',
    SUB_SURFACE: 'SubSurface',
    
    // Advanced
    CLIP_PLANES: 'ClipPlanes',
    FOG: 'Fog',
    LIGHT_INFORMATION: 'LightInformation',
    VIEW_DIRECTION: 'ViewDirection',
    SCENE_DEPTH: 'SceneDepth',
    SCREEN_SPACE_REFLECTION: 'ScreenSpaceReflection',
    CURRENT_SCREEN_BLOCK: 'CurrentScreenBlock',
    
    // Loop & Storage
    LOOP: 'Loop',
    STORAGE_READ: 'StorageRead',
    STORAGE_WRITE: 'StorageWrite',
    
    // Conditionals
    CONDITIONAL: 'Conditional',
    ELBOW: 'Elbow',
    
    // Image Processing
    WAVE: 'Wave',
    OSCILLATOR: 'Oscillator',
    
    // Custom
    CUSTOM: 'Custom',
    OPTIMIZERS: 'Optimizers'
  };

  // Initialize preview scene
  const initPreviewScene = () => {
    if (!previewCanvasRef) {
      console.error('No preview canvas ref found!');
      return;
    }
    
    console.log('Initializing preview scene...');
    
    previewEngine = new Engine(previewCanvasRef, true);
    previewScene = new Scene(previewEngine);
    previewScene.clearColor = new Color3(0.15, 0.15, 0.15);
    
    // Setup camera with arc rotation
    previewCamera = new ArcRotateCamera('previewCamera', Math.PI / 4, Math.PI / 3, 6, new Vector3(0, -0.5, 0), previewScene);
    previewCamera.attachControl(previewCanvasRef, true);
    
    // Set camera limits
    previewCamera.lowerRadiusLimit = 2;
    previewCamera.upperRadiusLimit = 10;
    
    // Set zoom speed (lower = slower)
    previewCamera.wheelDeltaPercentage = 0.01;
    
    // Setup lighting with shadows
    // Ambient lighting
    ambientLight = new HemisphericLight('ambientLight', new Vector3(0, 1, 0), previewScene);
    ambientLight.intensity = ambientIntensity();
    ambientLight.diffuse = new Color3(1, 1, 1);
    
    // Directional light for shadows
    directionalLight = new DirectionalLight('dirLight', new Vector3(-1, -1, -1), previewScene);
    directionalLight.position = new Vector3(3, 5, 3);
    directionalLight.intensity = lightIntensity();
    directionalLight.diffuse = new Color3(1, 1, 1);
    
    // Shadow generator
    shadowGenerator = new ShadowGenerator(shadowQuality(), directionalLight);
    shadowGenerator.useBlurExponentialShadowMap = true;
    shadowGenerator.blurKernel = 32;
    
    console.log('Scene created, camera and light set up');
    
    // Create backdrop and ground
    createBackdrop();
    
    // Create initial preview mesh
    updatePreviewMesh();
    
    // Start render loop
    previewEngine.runRenderLoop(() => {
      previewScene.render();
    });
    
    // Handle resize
    window.addEventListener('resize', () => {
      previewEngine.resize();
    });
    
    console.log('Preview scene initialization complete');
  };

  // Create backdrop and ground for better material preview
  const createBackdrop = () => {
    if (!previewScene) return;
    
    // Create ground plane with grid
    groundMesh = CreateGround('ground', { width: 10, height: 10 }, previewScene);
    groundMesh.position.y = -1.5;
    
    // Create grid material
    const gridMaterial = new StandardMaterial('gridMaterial', previewScene);
    gridMaterial.diffuseColor = new Color3(0.8, 0.8, 0.8);
    gridMaterial.specularColor = new Color3(0.1, 0.1, 0.1);
    
    // Create checkered grid pattern
    const checkerTexture = new Texture('data:image/svg+xml;base64,' + btoa(`
      <svg width="200" height="200" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <pattern id="checker" width="200" height="200" patternUnits="userSpaceOnUse">
            <rect x="0" y="0" width="100" height="100" fill="#f0f0f0"/>
            <rect x="100" y="100" width="100" height="100" fill="#f0f0f0"/>
            <rect x="100" y="0" width="100" height="100" fill="#ffffff"/>
            <rect x="0" y="100" width="100" height="100" fill="#ffffff"/>
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill="url(#checker)"/>
        <!-- Grid lines overlay -->
        <defs>
          <pattern id="gridlines" width="200" height="200" patternUnits="userSpaceOnUse">
            <path d="M 200 0 L 0 0 0 200" fill="none" stroke="#d0d0d0" stroke-width="0.5"/>
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill="url(#gridlines)"/>
      </svg>
    `), previewScene);
    
    checkerTexture.uScale = 3;
    checkerTexture.vScale = 4;
    gridMaterial.diffuseTexture = checkerTexture;
    groundMesh.material = gridMaterial;
    
    // Enable shadow receiving on ground
    groundMesh.receiveShadows = true;
    
    // Create backdrop sphere/dome for environment
    backdropMesh = CreateSphere('backdrop', { diameter: 20 }, previewScene);
    backdropMesh.position.y = 0;
    
    // Create backdrop material with gradient
    const backdropMaterial = new StandardMaterial('backdropMaterial', previewScene);
    backdropMaterial.diffuseColor = new Color3(0.95, 0.95, 1.0); // Slight blue tint
    backdropMaterial.specularColor = new Color3(0, 0, 0); // No specular
    backdropMaterial.backFaceCulling = false; // Render inside faces
    backdropMaterial.alpha = 0.3; // Semi-transparent
    backdropMesh.material = backdropMaterial;
    
    console.log('Created backdrop and grid');
  };

  // Update preview mesh based on selected shape
  const updatePreviewMesh = () => {
    if (!previewScene) {
      console.error('No preview scene for mesh creation!');
      return;
    }
    
    console.log('Creating preview mesh...');
    
    // Dispose existing mesh
    if (previewMesh) {
      previewMesh.dispose();
      console.log('Disposed existing mesh');
    }
    
    // Create new mesh based on shape and position it to rest on the ground plane
    // Ground plane is at Y = -1.5
    switch (previewShape()) {
      case 'sphere':
        previewMesh = CreateSphere('previewSphere', { diameter: 2 }, previewScene);
        previewMesh.position.y = -0.5; // Ground(-1.5) + radius(1) = -0.5
        break;
      case 'cube':
        previewMesh = CreateBox('previewCube', { size: 2 }, previewScene);
        previewMesh.position.y = -0.5; // Ground(-1.5) + half-height(1) = -0.5
        break;
      case 'plane':
        previewMesh = CreateGround('previewPlane', { width: 2, height: 2 }, previewScene);
        previewMesh.position.y = -0.5; // Plane at ground level
        previewMesh.rotation.x = Math.PI / 3; // Tilt slightly for better viewing
        break;
      case 'cylinder':
        previewMesh = CreateCylinder('previewCylinder', { height: 2, diameter: 1.5 }, previewScene);
        previewMesh.position.y = -0.5; // Ground(-1.5) + half-height(1) = -0.5
        break;
      case 'torus':
        previewMesh = CreateTorus('previewTorus', { diameter: 2, thickness: 0.5 }, previewScene);
        previewMesh.position.y = -0.5; // Center torus above ground
        break;
      default:
        previewMesh = CreateSphere('previewSphere', { diameter: 2 }, previewScene);
        previewMesh.position.y = -0.5;
    }
    
    console.log('Created mesh:', previewMesh);
    
    // Enable shadow casting on preview mesh
    if (shadowGenerator && previewMesh) {
      shadowGenerator.addShadowCaster(previewMesh);
    }
    
    // Apply current material if available
    if (currentMaterial()) {
      previewMesh.material = currentMaterial();
      console.log('Applied existing material to new mesh');
    } else {
      // Create a test material to ensure mesh is visible
      const testMaterial = new StandardMaterial('testMaterial', previewScene);
      testMaterial.diffuseColor = new Color3(0, 1, 0); // Bright green
      previewMesh.material = testMaterial;
      console.log('Applied test green material to mesh');
    }
    
    // Apply current settings to new mesh
    updateMeshScale();
    updateWireframe();
    updateAutoRotation();
  };

  // Initialize with default nodes
  const initializeDefaultNodes = () => {
    // Calculate center position based on viewport
    const centerX = nodeGraphRef ? (nodeGraphRef.clientWidth / 2) - 100 : 400; // Offset by half node width
    const centerY = nodeGraphRef ? (nodeGraphRef.clientHeight / 2) - 100 : 300; // Offset by half node height
    
    const outputNode = {
      id: 'material-output',
      type: NODE_TYPES.MATERIAL_OUTPUT,
      position: { x: centerX, y: centerY },
      title: 'Material Output',
      inputs: [
        { id: 'baseColor', name: 'Base Color', type: 'color', value: null },
        { id: 'roughness', name: 'Roughness', type: 'float', value: null },
        { id: 'metallic', name: 'Metallic', type: 'float', value: null },
        { id: 'normal', name: 'Normal', type: 'vector', value: null },
        { id: 'emissive', name: 'Emissive', type: 'color', value: null },
        { id: 'specular', name: 'Specular', type: 'color', value: null },
        { id: 'opacity', name: 'Opacity', type: 'float', value: null },
        { id: 'bump', name: 'Bump', type: 'float', value: null },
        { id: 'displacement', name: 'Displacement', type: 'float', value: null },
        { id: 'ambientOcclusion', name: 'AO', type: 'float', value: null }
      ],
      outputs: []
    };
    
    setNodes([outputNode]);
    createMaterialFromNodes();
  };

  // Helper function to construct texture URL from asset
  const constructTextureUrl = (asset) => {
    if (!asset) {
      console.log('constructTextureUrl: No asset provided');
      return null;
    }
    
    console.log('constructTextureUrl: Full asset object:', asset);
    
    let url = null;
    
    // Try multiple URL construction methods
    const possibleUrls = [];
    
    // Method 1: Use asset.path if it includes full project path
    if (asset.path && asset.path.includes('projects/')) {
      const cleanPath = asset.path.startsWith('/') ? asset.path.slice(1) : asset.path;
      possibleUrls.push(`http://localhost:3001/file/${cleanPath}`);
    }
    
    // Method 2: Use asset.path directly with projects/test prefix
    if (asset.path) {
      const cleanPath = asset.path.startsWith('/') ? asset.path.slice(1) : asset.path;
      possibleUrls.push(`http://localhost:3001/file/projects/test/${cleanPath}`);
    }
    
    // Method 3: Try with assets subdirectory
    if (asset.name || asset.id) {
      const fileName = asset.name || asset.id;
      possibleUrls.push(`http://localhost:3001/file/projects/test/assets/${fileName}`);
      possibleUrls.push(`http://localhost:3001/file/projects/test/assets/textures/${fileName}`);
      possibleUrls.push(`http://localhost:3001/file/projects/test/assets/hdri/${fileName}`);
      possibleUrls.push(`http://localhost:3001/file/projects/test/assets/images/${fileName}`);
    }
    
    // Method 4: Try the exact same pattern as thumbnail URLs work
    if (asset.thumbnailUrl) {
      console.log('constructTextureUrl: Found thumbnail URL pattern:', asset.thumbnailUrl);
      // Extract the base path from thumbnail URL and construct full image URL
      // Thumbnail: http://localhost:3001/file/projects/test/.cache/thumbnails/hills_hdr_256.png
      // Should be: http://localhost:3001/file/projects/test/assets/materials/hills.hdr
      
      // Extract the base URL up to the project directory
      const baseProjectUrl = asset.thumbnailUrl.split('/.cache/')[0]; // http://localhost:3001/file/projects/test
      
      // Use the asset path to construct the full URL
      if (asset.path) {
        const fullUrl = `${baseProjectUrl}/${asset.path}`;
        possibleUrls.unshift(fullUrl); // Add to beginning as most likely to work
        console.log('constructTextureUrl: Constructed from thumbnail pattern:', fullUrl);
      }
    }
    
    console.log('constructTextureUrl: Possible URLs to try:', possibleUrls);
    
    // Return the first URL (we'll implement fallback later if needed)
    url = possibleUrls[0] || null;
    
    console.log('constructTextureUrl: Using URL:', url);
    return url;
  };

  // Helper function to create texture with proper format handling
  const createTextureFromAsset = (asset, scene) => {
    const textureUrl = constructTextureUrl(asset);
    if (!textureUrl) return null;

    // Check file extension to determine if special handling is needed
    const extension = asset.extension?.toLowerCase() || 
                    asset.name?.split('.').pop()?.toLowerCase() || 
                    textureUrl.split('.').pop()?.toLowerCase();

    console.log(`Creating texture for ${asset.name} with extension: ${extension}`);

    // Create texture - Babylon.js should automatically handle EXR with the loader imported
    const texture = new Texture(textureUrl, scene);

    // Special handling for HDR formats like EXR
    if (extension === 'exr' || extension === 'hdr') {
      // Set proper texture format for HDR
      texture.gammaSpace = false; // HDR textures are in linear space
      texture.level = 1.0; // Set exposure level
      
      // For normal maps and other data textures, ensure proper handling
      if (asset.name?.toLowerCase().includes('normal') || asset.name?.toLowerCase().includes('nor')) {
        texture.gammaSpace = false; // Normal maps should always be linear
      }
    }

    // Add error and load handling
    texture.onError = () => {
      console.error(`Failed to load ${extension?.toUpperCase()} texture:`, textureUrl);
      if (extension === 'exr') {
        console.log('Note: EXR files require proper server MIME type configuration');
        console.log('If sphere disappears, try using a lower exposure or different texture');
      }
    };

    texture.onLoad = () => {
      console.log(`✅ ${extension?.toUpperCase()} texture loaded successfully:`, textureUrl);
      if (extension === 'exr') {
        console.log('EXR texture supports HDR data - check material properties if sphere disappears');
      }
    };

    return texture;
  };

  // Create material from node graph - supports both Standard and PBR materials
  const createMaterialFromNodes = () => {
    const scene = previewScene;
    if (!scene) return;
    
    // Choose material type based on PBR setting
    const material = usePBR() 
      ? new PBRMaterial('NodeBasedPBRMaterial', scene)
      : new StandardMaterial('NodeBasedMaterial', scene);
    
    // Set some defaults that will be visible
    if (usePBR()) {
      // PBR Material defaults
      material.baseColor = new Color3(0.8, 0.8, 0.8);
      material.metallicFactor = 0.0;
      material.roughnessFactor = 0.5;
    } else {
      // Standard Material defaults
      material.diffuseColor = new Color3(0.8, 0.8, 0.8);
      material.specularColor = new Color3(0.2, 0.2, 0.2);
    }
    
    console.log('Default material created:', usePBR() ? 'PBR' : 'Standard');
    
    // Handle all material property connections
    connections().forEach(connection => {
      if (connection.to.nodeId !== 'material-output') return;
      
      const sourceNode = nodes().find(n => n.id === connection.from.nodeId);
      if (!sourceNode) return;
      
      switch (connection.to.socketId) {
        case 'baseColor':
          if (sourceNode.type === NODE_TYPES.COLOR) {
            const colorInput = sourceNode.inputs.find(i => i.id === 'color');
            if (colorInput?.value && colorInput.value instanceof Color3) {
              if (usePBR()) {
                material.baseColor = colorInput.value;
              } else {
                material.diffuseColor = colorInput.value;
              }
              console.log('Applied base color:', colorInput.value);
            }
          } else if (sourceNode.type === NODE_TYPES.TEXTURE_SAMPLE) {
            // Handle texture connection to base color
            const textureInput = sourceNode.inputs.find(i => i.id === 'texture');
            if (textureInput?.value) {
              const asset = textureInput.value;
              const texture = createTextureFromAsset(asset, scene);
              if (texture) {
                if (usePBR()) {
                  material.baseTexture = texture;
                  material.baseColor = new Color3(1.0, 1.0, 1.0);
                } else {
                  material.diffuseTexture = texture;
                  material.diffuseColor = new Color3(1.0, 1.0, 1.0);
                }
                
                // Special handling for HDR textures on base color
                const extension = asset.extension?.toLowerCase() || asset.name?.split('.').pop()?.toLowerCase();
                if (extension === 'exr' || extension === 'hdr') {
                  // For HDR textures on base color, ensure material doesn't become transparent
                  material.alpha = 1.0;
                  if (usePBR()) {
                    material.baseColor = new Color3(0.5, 0.5, 0.5);
                  } else {
                    material.diffuseColor = new Color3(0.5, 0.5, 0.5);
                  }
                  console.log('Applied HDR texture to base color with exposure compensation');
                } else {
                  console.log('Applied texture to base color');
                }
              }
            }
          }
          break;
          
          
        case 'emissive':
          if (sourceNode.type === NODE_TYPES.COLOR) {
            const colorInput = sourceNode.inputs.find(i => i.id === 'color');
            if (colorInput?.value && colorInput.value instanceof Color3) {
              material.emissiveColor = colorInput.value;
              console.log('Applied emissive color:', colorInput.value);
            }
          } else if (sourceNode.type === NODE_TYPES.TEXTURE_SAMPLE) {
            const textureInput = sourceNode.inputs.find(i => i.id === 'texture');
            if (textureInput?.value) {
              const asset = textureInput.value;
              const texture = createTextureFromAsset(asset, scene);
              if (texture) {
                material.emissiveTexture = texture;
                console.log('Applied emissive texture');
              }
            }
          }
          break;
          
        case 'normal':
          if (sourceNode.type === NODE_TYPES.TEXTURE_SAMPLE) {
            const textureInput = sourceNode.inputs.find(i => i.id === 'texture');
            if (textureInput?.value) {
              const asset = textureInput.value;
              const texture = createTextureFromAsset(asset, scene);
              if (texture) {
                material.bumpTexture = texture;
                material.useParallax = false; // Use normal mapping
                console.log('Applied normal texture');
              }
            }
          }
          break;
          
        case 'roughness':
          if (sourceNode.type === NODE_TYPES.TEXTURE_SAMPLE) {
            const textureInput = sourceNode.inputs.find(i => i.id === 'texture');
            if (textureInput?.value) {
              const asset = textureInput.value;
              const texture = createTextureFromAsset(asset, scene);
              if (texture) {
                if (usePBR()) {
                  material.metallicRoughnessTexture = texture;
                  material.useRoughnessFromMetallicTextureGreen = true;
                } else {
                  material.specularTexture = texture;
                }
                console.log('Applied roughness texture');
              }
            }
          } else if (sourceNode.type === NODE_TYPES.CONSTANT) {
            const valueInput = sourceNode.inputs.find(i => i.id === 'value');
            if (valueInput?.value !== undefined) {
              if (usePBR()) {
                material.roughnessFactor = valueInput.value;
              } else {
                // For StandardMaterial, we use specularPower (inverse relationship)
                material.specularPower = Math.max(1, (1 - valueInput.value) * 128);
              }
              console.log('Applied roughness:', valueInput.value);
            }
          }
          break;
          
        case 'metallic':
          if (sourceNode.type === NODE_TYPES.TEXTURE_SAMPLE) {
            const textureInput = sourceNode.inputs.find(i => i.id === 'texture');
            if (textureInput?.value) {
              const asset = textureInput.value;
              const texture = createTextureFromAsset(asset, scene);
              if (texture) {
                if (usePBR()) {
                  material.metallicRoughnessTexture = texture;
                  material.useMetallnessFromMetallicTextureBlue = true;
                } else {
                  material.reflectionTexture = texture;
                }
                console.log('Applied metallic texture');
              }
            }
          } else if (sourceNode.type === NODE_TYPES.CONSTANT) {
            const valueInput = sourceNode.inputs.find(i => i.id === 'value');
            if (valueInput?.value !== undefined) {
              const metallic = valueInput.value;
              if (usePBR()) {
                material.metallicFactor = metallic;
              } else {
                material.specularColor = new Color3(metallic, metallic, metallic);
              }
              console.log('Applied metallic:', metallic);
            }
          }
          break;
          
        case 'specular':
          if (sourceNode.type === NODE_TYPES.COLOR) {
            const colorInput = sourceNode.inputs.find(i => i.id === 'color');
            if (colorInput?.value && colorInput.value instanceof Color3) {
              material.specularColor = colorInput.value;
              console.log('Applied specular color:', colorInput.value);
            }
          } else if (sourceNode.type === NODE_TYPES.TEXTURE_SAMPLE) {
            const textureInput = sourceNode.inputs.find(i => i.id === 'texture');
            if (textureInput?.value) {
              const asset = textureInput.value;
              const texture = createTextureFromAsset(asset, scene);
              if (texture) {
                material.specularTexture = texture;
                console.log('Applied specular texture');
              }
            }
          }
          break;
          
        case 'opacity':
          if (sourceNode.type === NODE_TYPES.CONSTANT) {
            const valueInput = sourceNode.inputs.find(i => i.id === 'value');
            if (valueInput?.value !== undefined) {
              material.alpha = valueInput.value;
              console.log('Applied opacity:', valueInput.value);
            }
          } else if (sourceNode.type === NODE_TYPES.TEXTURE_SAMPLE) {
            const textureInput = sourceNode.inputs.find(i => i.id === 'texture');
            if (textureInput?.value) {
              const asset = textureInput.value;
              const texture = createTextureFromAsset(asset, scene);
              if (texture) {
                material.opacityTexture = texture;
                console.log('Applied opacity texture');
              }
            }
          }
          break;
          
        case 'bump':
          if (sourceNode.type === NODE_TYPES.TEXTURE_SAMPLE) {
            const textureInput = sourceNode.inputs.find(i => i.id === 'texture');
            if (textureInput?.value) {
              const asset = textureInput.value;
              const texture = createTextureFromAsset(asset, scene);
              if (texture) {
                material.bumpTexture = texture;
                material.useParallax = true; // Use parallax mapping for bump
                console.log('Applied bump texture');
              }
            }
          }
          break;
          
        case 'ambientOcclusion':
          if (sourceNode.type === NODE_TYPES.TEXTURE_SAMPLE) {
            const textureInput = sourceNode.inputs.find(i => i.id === 'texture');
            if (textureInput?.value) {
              const asset = textureInput.value;
              const texture = createTextureFromAsset(asset, scene);
              if (texture) {
                material.ambientTexture = texture;
                console.log('Applied AO texture');
              }
            }
          }
          break;
      }
    });
    
    setCurrentMaterial(material);
    
    // Apply to preview mesh
    if (previewMesh) {
      previewMesh.material = material;
      console.log('Material applied to mesh - color should be visible now');
      console.log('Mesh material:', previewMesh.material);
      console.log('Material baseColor:', material.baseColor);
      console.log('Scene:', previewScene);
      console.log('Mesh:', previewMesh);
    } else {
      console.error('No preview mesh found!');
    }
  };

  // Calculate center position for new nodes
  const getCenterPosition = () => {
    if (!nodeGraphRef) return { x: 400, y: 300 };
    
    const rect = nodeGraphRef.getBoundingClientRect();
    const currentPan = pan();
    const currentZoom = zoom();
    
    // Calculate center in graph coordinates
    const centerX = (rect.width / 2 - currentPan.x) / currentZoom;
    const centerY = (rect.height / 2 - currentPan.y) / currentZoom;
    
    // Offset slightly to avoid overlapping the existing material output node
    return { 
      x: centerX - 100, // Offset left by 100px
      y: centerY - 50   // Offset up by 50px
    };
  };

  // Add new node
  const addNode = (type, position, asset = null) => {
    const nodeId = `node-${Date.now()}`;
    let newNode;
    
    switch (type) {
      // Output Blocks
      case NODE_TYPES.VERTEX_OUTPUT:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Vertex Output',
          inputs: [
            { id: 'vector', name: 'Vector', type: 'vector4', value: null }
          ],
          outputs: []
        };
        break;
        
      case NODE_TYPES.FRAGMENT_OUTPUT:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Fragment Output',
          inputs: [
            { id: 'rgba', name: 'RGBA', type: 'color', value: null },
            { id: 'rgb', name: 'RGB', type: 'color', value: null },
            { id: 'a', name: 'A', type: 'float', value: 1.0 }
          ],
          outputs: []
        };
        break;
        
      case NODE_TYPES.DISCARD:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Discard',
          inputs: [
            { id: 'value', name: 'Value', type: 'float', value: 0.0 }
          ],
          outputs: []
        };
        break;
        
      // Input Blocks
      case NODE_TYPES.FLOAT:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Float',
          inputs: [
            { id: 'value', name: 'Value', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.VECTOR2:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Vector2',
          inputs: [
            { id: 'x', name: 'X', type: 'float', value: 0.0 },
            { id: 'y', name: 'Y', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'xy', name: 'XY', type: 'vector2' },
            { id: 'x', name: 'X', type: 'float' },
            { id: 'y', name: 'Y', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.VECTOR3:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Vector3',
          inputs: [
            { id: 'x', name: 'X', type: 'float', value: 0.0 },
            { id: 'y', name: 'Y', type: 'float', value: 0.0 },
            { id: 'z', name: 'Z', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'xyz', name: 'XYZ', type: 'vector3' },
            { id: 'x', name: 'X', type: 'float' },
            { id: 'y', name: 'Y', type: 'float' },
            { id: 'z', name: 'Z', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.VECTOR4:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Vector4',
          inputs: [
            { id: 'x', name: 'X', type: 'float', value: 0.0 },
            { id: 'y', name: 'Y', type: 'float', value: 0.0 },
            { id: 'z', name: 'Z', type: 'float', value: 0.0 },
            { id: 'w', name: 'W', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'xyzw', name: 'XYZW', type: 'vector4' },
            { id: 'xyz', name: 'XYZ', type: 'vector3' },
            { id: 'x', name: 'X', type: 'float' },
            { id: 'y', name: 'Y', type: 'float' },
            { id: 'z', name: 'Z', type: 'float' },
            { id: 'w', name: 'W', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.COLOR3:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Color3',
          inputs: [
            { id: 'color', name: 'Color', type: 'color3', value: new Color3(1.0, 1.0, 1.0) }
          ],
          outputs: [
            { id: 'rgb', name: 'RGB', type: 'color3' },
            { id: 'r', name: 'R', type: 'float' },
            { id: 'g', name: 'G', type: 'float' },
            { id: 'b', name: 'B', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.COLOR4:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Color4',
          inputs: [
            { id: 'color', name: 'Color', type: 'color4', value: new Color3(1.0, 1.0, 1.0) }
          ],
          outputs: [
            { id: 'rgba', name: 'RGBA', type: 'color4' },
            { id: 'rgb', name: 'RGB', type: 'color3' },
            { id: 'r', name: 'R', type: 'float' },
            { id: 'g', name: 'G', type: 'float' },
            { id: 'b', name: 'B', type: 'float' },
            { id: 'a', name: 'A', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.TEXTURE:
      case NODE_TYPES.TEXTURE_SAMPLE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: asset ? asset.name || 'Texture Sample' : 'Texture Sample',
          asset: asset, // Store the asset data for preview
          inputs: [
            { id: 'texture', name: 'Texture', type: 'texture', value: asset },
            { id: 'uv', name: 'UV', type: 'vector2', value: null }
          ],
          outputs: [
            { id: 'rgb', name: 'RGB', type: 'color' },
            { id: 'r', name: 'R', type: 'float' },
            { id: 'g', name: 'G', type: 'float' },
            { id: 'b', name: 'B', type: 'float' },
            { id: 'a', name: 'A', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.CONSTANT:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Constant',
          inputs: [
            { id: 'value', name: 'Value', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.COLOR:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Color',
          inputs: [
            { id: 'color', name: 'Color', type: 'color', value: new Color3(1.0, 1.0, 1.0) }
          ],
          outputs: [
            { id: 'rgb', name: 'RGB', type: 'color' },
            { id: 'r', name: 'R', type: 'float' },
            { id: 'g', name: 'G', type: 'float' },
            { id: 'b', name: 'B', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.MULTIPLY:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Multiply',
          inputs: [
            { id: 'left', name: 'A', type: 'float', value: 1.0 },
            { id: 'right', name: 'B', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.ADD:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Add',
          inputs: [
            { id: 'left', name: 'A', type: 'float', value: 0.0 },
            { id: 'right', name: 'B', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.LERP:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Lerp',
          inputs: [
            { id: 'left', name: 'A', type: 'color', value: new Color3(0.0, 0.0, 0.0) },
            { id: 'right', name: 'B', type: 'color', value: new Color3(1.0, 1.0, 1.0) },
            { id: 'gradient', name: 'Factor', type: 'float', value: 0.5 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'color' }
          ]
        };
        break;
      case NODE_TYPES.FRESNEL:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Fresnel',
          inputs: [
            { id: 'bias', name: 'Bias', type: 'float', value: 0.0 },
            { id: 'scale', name: 'Scale', type: 'float', value: 1.0 },
            { id: 'power', name: 'Power', type: 'float', value: 5.0 }
          ],
          outputs: [
            { id: 'fresnel', name: 'Fresnel', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.CLAMP:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Clamp',
          inputs: [
            { id: 'value', name: 'Value', type: 'float', value: 0.5 },
            { id: 'min', name: 'Min', type: 'float', value: 0.0 },
            { id: 'max', name: 'Max', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.POWER:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Power',
          inputs: [
            { id: 'value', name: 'Value', type: 'float', value: 0.5 },
            { id: 'power', name: 'Power', type: 'float', value: 2.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.SUBTRACT:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Subtract',
          inputs: [
            { id: 'left', name: 'A', type: 'float', value: 1.0 },
            { id: 'right', name: 'B', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.DIVIDE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Divide',
          inputs: [
            { id: 'left', name: 'A', type: 'float', value: 1.0 },
            { id: 'right', name: 'B', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.TIME:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Time',
          inputs: [],
          outputs: [
            { id: 'time', name: 'Time', type: 'float' },
            { id: 'sine', name: 'Sine Time', type: 'float' },
            { id: 'cosine', name: 'Cosine Time', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.UV_COORDINATES:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'UV Coordinates',
          inputs: [],
          outputs: [
            { id: 'uv', name: 'UV', type: 'vector2' },
            { id: 'u', name: 'U', type: 'float' },
            { id: 'v', name: 'V', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.NORMAL_MAP:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Normal Map',
          inputs: [
            { id: 'normalTexture', name: 'Normal Texture', type: 'texture', value: null },
            { id: 'strength', name: 'Strength', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'normal', name: 'Normal', type: 'vector' }
          ]
        };
        break;
      case NODE_TYPES.DOT_PRODUCT:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Dot Product',
          inputs: [
            { id: 'vectorA', name: 'Vector A', type: 'vector', value: null },
            { id: 'vectorB', name: 'Vector B', type: 'vector', value: null }
          ],
          outputs: [
            { id: 'result', name: 'Result', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.NOISE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Noise',
          inputs: [
            { id: 'coordinates', name: 'Coordinates', type: 'vector2', value: null },
            { id: 'scale', name: 'Scale', type: 'float', value: 1.0 },
            { id: 'detail', name: 'Detail', type: 'float', value: 2.0 }
          ],
          outputs: [
            { id: 'noise', name: 'Noise', type: 'float' },
            { id: 'color', name: 'Color', type: 'color' }
          ]
        };
        break;
      case NODE_TYPES.SATURATE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Saturate',
          inputs: [
            { id: 'value', name: 'Value', type: 'float', value: 0.5 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.ABS:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Absolute',
          inputs: [
            { id: 'value', name: 'Value', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.SIN:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Sine',
          inputs: [
            { id: 'value', name: 'Value', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
      case NODE_TYPES.COS:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Cosine',
          inputs: [
            { id: 'value', name: 'Value', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
        
      // Input Nodes
      case NODE_TYPES.CAMERA_POSITION:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Camera Position',
          inputs: [],
          outputs: [
            { id: 'position', name: 'Position', type: 'vector' }
          ]
        };
        break;
        
      case NODE_TYPES.VIEW_DIRECTION:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'View Direction',
          inputs: [],
          outputs: [
            { id: 'direction', name: 'Direction', type: 'vector' }
          ]
        };
        break;
        
      case NODE_TYPES.WORLD_POSITION:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'World Position',
          inputs: [],
          outputs: [
            { id: 'position', name: 'Position', type: 'vector' }
          ]
        };
        break;
        
      case NODE_TYPES.WORLD_NORMAL:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'World Normal',
          inputs: [],
          outputs: [
            { id: 'normal', name: 'Normal', type: 'vector' }
          ]
        };
        break;
        
      case NODE_TYPES.SCREEN_SIZE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Screen Size',
          inputs: [],
          outputs: [
            { id: 'size', name: 'Size', type: 'vector2' },
            { id: 'width', name: 'Width', type: 'float' },
            { id: 'height', name: 'Height', type: 'float' }
          ]
        };
        break;
        
      // Math - Basic Operations
      case NODE_TYPES.SUBTRACT:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Subtract',
          inputs: [
            { id: 'left', name: 'A', type: 'float', value: 1.0 },
            { id: 'right', name: 'B', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.DIVIDE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Divide',
          inputs: [
            { id: 'left', name: 'A', type: 'float', value: 1.0 },
            { id: 'right', name: 'B', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.SCALE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Scale',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 1.0 },
            { id: 'factor', name: 'Factor', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.NEGATE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Negate',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.ONE_MINUS:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'One Minus',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.RECIPROCAL:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Reciprocal',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      // Math - Range Operations
      case NODE_TYPES.NORMALIZE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Normalize',
          inputs: [
            { id: 'input', name: 'Input', type: 'vector', value: null }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'vector' }
          ]
        };
        break;
        
      case NODE_TYPES.REMAP:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Remap',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 0.5 },
            { id: 'sourceMin', name: 'Source Min', type: 'float', value: 0.0 },
            { id: 'sourceMax', name: 'Source Max', type: 'float', value: 1.0 },
            { id: 'targetMin', name: 'Target Min', type: 'float', value: 0.0 },
            { id: 'targetMax', name: 'Target Max', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.STEP:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Step',
          inputs: [
            { id: 'edge', name: 'Edge', type: 'float', value: 0.5 },
            { id: 'input', name: 'Input', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.SMOOTHSTEP:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Smoothstep',
          inputs: [
            { id: 'min', name: 'Min', type: 'float', value: 0.0 },
            { id: 'max', name: 'Max', type: 'float', value: 1.0 },
            { id: 'input', name: 'Input', type: 'float', value: 0.5 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      // Math - Trigonometry
      case NODE_TYPES.TAN:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Tangent',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.ASIN:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Arcsine',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.ACOS:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Arccosine',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.ATAN:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Arctangent',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.ATAN2:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Arctangent2',
          inputs: [
            { id: 'y', name: 'Y', type: 'float', value: 1.0 },
            { id: 'x', name: 'X', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      // Math - Vector Operations
      case NODE_TYPES.DISTANCE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Distance',
          inputs: [
            { id: 'left', name: 'A', type: 'vector', value: null },
            { id: 'right', name: 'B', type: 'vector', value: null }
          ],
          outputs: [
            { id: 'output', name: 'Distance', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.LENGTH:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Length',
          inputs: [
            { id: 'input', name: 'Input', type: 'vector', value: null }
          ],
          outputs: [
            { id: 'output', name: 'Length', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.REFLECT:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Reflect',
          inputs: [
            { id: 'incident', name: 'Incident', type: 'vector', value: null },
            { id: 'normal', name: 'Normal', type: 'vector', value: null }
          ],
          outputs: [
            { id: 'output', name: 'Reflection', type: 'vector' }
          ]
        };
        break;
        
      case NODE_TYPES.REFRACT:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Refract',
          inputs: [
            { id: 'incident', name: 'Incident', type: 'vector', value: null },
            { id: 'normal', name: 'Normal', type: 'vector', value: null },
            { id: 'eta', name: 'IOR', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Refraction', type: 'vector' }
          ]
        };
        break;
        
      // Math - Rounding
      case NODE_TYPES.SIGN:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Sign',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.ROUND:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Round',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 0.5 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.MOD:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Modulo',
          inputs: [
            { id: 'left', name: 'A', type: 'float', value: 1.0 },
            { id: 'right', name: 'B', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
        
      // Math - Exponential
      case NODE_TYPES.EXP:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Exponential',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.EXP2:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Exponential2',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.LOG:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Logarithm',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.LOG2:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Logarithm2',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.SQRT:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Square Root',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 4.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      // Math - Comparison
      case NODE_TYPES.MIN:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Minimum',
          inputs: [
            { id: 'left', name: 'A', type: 'float', value: 0.0 },
            { id: 'right', name: 'B', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.MAX:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Maximum',
          inputs: [
            { id: 'left', name: 'A', type: 'float', value: 0.0 },
            { id: 'right', name: 'B', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.LESS_THAN:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Less Than',
          inputs: [
            { id: 'left', name: 'A', type: 'float', value: 0.0 },
            { id: 'right', name: 'B', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.GREATER_THAN:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Greater Than',
          inputs: [
            { id: 'left', name: 'A', type: 'float', value: 1.0 },
            { id: 'right', name: 'B', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Result', type: 'float' }
          ]
        };
        break;
        
      // Utilities
      case NODE_TYPES.VORONOI_NOISE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Voronoi Noise',
          inputs: [
            { id: 'uv', name: 'UV', type: 'vector2', value: null },
            { id: 'scale', name: 'Scale', type: 'float', value: 5.0 },
            { id: 'offset', name: 'Offset', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' },
            { id: 'cells', name: 'Cells', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.WAVE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Wave',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 0.0 },
            { id: 'frequency', name: 'Frequency', type: 'float', value: 1.0 },
            { id: 'amplitude', name: 'Amplitude', type: 'float', value: 1.0 },
            { id: 'offset', name: 'Offset', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'sine', name: 'Sine', type: 'float' },
            { id: 'cosine', name: 'Cosine', type: 'float' },
            { id: 'sawtooth', name: 'Sawtooth', type: 'float' },
            { id: 'triangle', name: 'Triangle', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.DESATURATE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Desaturate',
          inputs: [
            { id: 'color', name: 'Color', type: 'color', value: new Color3(1.0, 0.5, 0.0) },
            { id: 'level', name: 'Level', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'color' }
          ]
        };
        break;
        
      case NODE_TYPES.POSTERIZE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Posterize',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 0.5 },
            { id: 'steps', name: 'Steps', type: 'float', value: 4.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.CONDITIONAL:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Conditional',
          inputs: [
            { id: 'condition', name: 'Condition', type: 'float', value: 0.0 },
            { id: 'true', name: 'True', type: 'float', value: 1.0 },
            { id: 'false', name: 'False', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.HUE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Hue',
          inputs: [
            { id: 'color', name: 'Color', type: 'color', value: new Color3(1.0, 0.5, 0.0) },
            { id: 'hue', name: 'Hue', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'color' }
          ]
        };
        break;
        
      case NODE_TYPES.SATURATION:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Saturation',
          inputs: [
            { id: 'color', name: 'Color', type: 'color', value: new Color3(1.0, 0.5, 0.0) },
            { id: 'saturation', name: 'Saturation', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'color' }
          ]
        };
        break;
        
      case NODE_TYPES.CONTRAST:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Contrast',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 0.5 },
            { id: 'contrast', name: 'Contrast', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.BRIGHTNESS:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Brightness',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 0.5 },
            { id: 'brightness', name: 'Brightness', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.INVERT:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Invert',
          inputs: [
            { id: 'input', name: 'Input', type: 'float', value: 0.5 }
          ],
          outputs: [
            { id: 'output', name: 'Output', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.VERTEX_COLOR:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Vertex Color',
          inputs: [],
          outputs: [
            { id: 'color', name: 'Color', type: 'color' },
            { id: 'r', name: 'R', type: 'float' },
            { id: 'g', name: 'G', type: 'float' },
            { id: 'b', name: 'B', type: 'float' },
            { id: 'a', name: 'A', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.TEXTURE_COORDINATE:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'Texture Coordinate',
          inputs: [
            { id: 'index', name: 'Index', type: 'float', value: 0.0 }
          ],
          outputs: [
            { id: 'uv', name: 'UV', type: 'vector2' },
            { id: 'u', name: 'U', type: 'float' },
            { id: 'v', name: 'V', type: 'float' }
          ]
        };
        break;
        
      case NODE_TYPES.PBR_METALLIC_ROUGHNESS:
        newNode = {
          id: nodeId,
          type,
          position,
          title: 'PBR Metallic Roughness',
          inputs: [
            { id: 'baseColor', name: 'Base Color', type: 'color', value: new Color3(0.8, 0.8, 0.8) },
            { id: 'metallic', name: 'Metallic', type: 'float', value: 0.0 },
            { id: 'roughness', name: 'Roughness', type: 'float', value: 0.5 },
            { id: 'normal', name: 'Normal', type: 'vector', value: null },
            { id: 'emissive', name: 'Emissive', type: 'color', value: new Color3(0.0, 0.0, 0.0) },
            { id: 'occlusion', name: 'Occlusion', type: 'float', value: 1.0 }
          ],
          outputs: [
            { id: 'material', name: 'Material', type: 'material' }
          ]
        };
        break;
    }
    
    if (newNode) {
      setNodes(prev => [...prev, newNode]);
      // Bring newly created node to the front
      bringNodeToFront(newNode.id);
    }
  };

  // Handle node drag
  // Function to get z-index for a node
  const getNodeZIndex = (nodeId) => {
    const zIndices = nodeZIndices();
    return zIndices.get(nodeId) || 1;
  };

  // Function to bring a node to the front
  const bringNodeToFront = (nodeId) => {
    const currentMax = maxZIndex();
    const newZIndex = currentMax + 1;
    setMaxZIndex(newZIndex);
    setNodeZIndices(prev => {
      const newMap = new Map(prev);
      newMap.set(nodeId, newZIndex);
      return newMap;
    });
  };

  const handleNodeMouseDown = (e, node) => {
    e.preventDefault();
    e.stopPropagation();
    
    const rect = nodeGraphRef.getBoundingClientRect();
    // Convert screen coordinates to node graph coordinates
    const screenX = e.clientX - rect.left;
    const screenY = e.clientY - rect.top;
    const graphX = (screenX - pan().x) / zoom();
    const graphY = (screenY - pan().y) / zoom();
    
    // Bring the node to front when starting to drag
    bringNodeToFront(node.id);
    
    setDraggedNode(node);
    setDragOffset({
      x: graphX - node.position.x,
      y: graphY - node.position.y
    });
    
    // Initialize drag transform with current position to prevent jump
    setDraggedNodeTransform({ x: node.position.x, y: node.position.y });
    
    setSelectedNode(node);
  };

  const handleMouseMove = (e) => {
    if (draggedNode()) {
      const rect = nodeGraphRef.getBoundingClientRect();
      const newX = (e.clientX - rect.left - pan().x) / zoom() - dragOffset().x;
      const newY = (e.clientY - rect.top - pan().y) / zoom() - dragOffset().y;
      
      // Just update the transform, don't re-render the entire node
      setDraggedNodeTransform({ x: newX, y: newY });
    } else if (isDraggingAllNodes()) {
      handleAllNodesDragMove(e);
    } else if (isPanning()) {
      handlePanMove(e);
    } else if (draggingConnection()) {
      const rect = nodeGraphRef.getBoundingClientRect();
      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;
      
      // Store screen coordinates for SVG rendering
      setDragConnectionEnd({ x: screenX, y: screenY });
    }
  };

  // Handle dragging all nodes
  const handleAllNodesDragMove = (e) => {
    const deltaX = (e.clientX - allNodesDragStart().x) / zoom();
    const deltaY = (e.clientY - allNodesDragStart().y) / zoom();
    
    setNodes(prev => prev.map(node => ({
      ...node,
      position: {
        x: node.position.x + deltaX,
        y: node.position.y + deltaY
      }
    })));
    
    setAllNodesDragStart({ x: e.clientX, y: e.clientY });
    setSocketPositionCache(new Map()); // Clear cache to update connections
  };

  const handleMouseUp = () => {
    const wasDragging = !!draggedNode();
    
    // Commit the final position to the nodes signal when drag ends
    if (wasDragging) {
      const transform = draggedNodeTransform();
      setNodes(prev => prev.map(node => 
        node.id === draggedNode().id 
          ? { ...node, position: { x: transform.x, y: transform.y } }
          : node
      ));
      setSocketPositionCache(new Map()); // Clear cache after position change
    }
    
    setDraggedNode(null);
    setIsPanning(false);
    setIsDraggingAllNodes(false);
    
    // Handle connection drop
    if (draggingConnection()) {
      const hovered = hoveredSocket();
      if (hovered && canConnect(draggingConnection(), hovered)) {
        addConnection(draggingConnection(), hovered);
      }
      setDraggingConnection(null);
      setHoveredSocket(null);
    }
  };

  // Connection management with type checking
  const canConnect = (from, to) => {
    // Can't connect to same node
    if (from.nodeId === to.nodeId) return false;
    
    // Can only connect output to input
    if (from.type !== 'output' || to.type !== 'input') return false;
    
    // Check if connection already exists
    const existingConnection = connections().some(conn => 
      conn.from.nodeId === from.nodeId && conn.from.socketId === from.socketId &&
      conn.to.nodeId === to.nodeId && conn.to.socketId === to.socketId
    );
    if (existingConnection) return false;
    
    // Type checking
    const fromNode = nodes().find(n => n.id === from.nodeId);
    const toNode = nodes().find(n => n.id === to.nodeId);
    if (!fromNode || !toNode) return false;
    
    const fromSocket = fromNode.outputs?.find(s => s.id === from.socketId);
    const toSocket = toNode.inputs?.find(s => s.id === to.socketId);
    if (!fromSocket || !toSocket) return false;
    
    // Compatible types: exact match, or float can connect to any numeric input
    const compatibleTypes = [
      [fromSocket.type, toSocket.type], // exact match
      ['float', 'color'], // float can connect to color components
      ['color', 'float'], // color can connect to float (uses magnitude)
      ['vector', 'float'], // vector can connect to float
      ['texture', 'color'] // texture can provide color
    ];
    
    return compatibleTypes.some(([from, to]) => 
      fromSocket.type === from && toSocket.type === to
    );
  };

  const addConnection = (from, to) => {
    const newConnection = {
      id: `conn-${Date.now()}`,
      from,
      to
    };
    setConnections(prev => [...prev, newConnection]);
    createMaterialFromNodes();
  };

  const removeConnection = (connectionId) => {
    console.log('removeConnection called with id:', connectionId);
    setConnections(prev => {
      const newConnections = prev.filter(conn => conn.id !== connectionId);
      console.log('Connections before removal:', prev.length, 'after:', newConnections.length);
      return newConnections;
    });
    createMaterialFromNodes();
  };

  // Remove selected node
  const removeNode = (nodeId) => {
    console.log('removeNode called with nodeId:', nodeId);
    
    // Don't allow removing the material output node
    if (nodeId === 'material-output') {
      console.log('Cannot remove material output node');
      return;
    }
    
    console.log('Removing node and its connections...');
    
    // Remove the node
    setNodes(prev => {
      const newNodes = prev.filter(node => node.id !== nodeId);
      console.log('Nodes after removal:', newNodes.length);
      return newNodes;
    });
    
    // Remove all connections involving this node
    setConnections(prev => {
      const newConnections = prev.filter(conn => 
        conn.from.nodeId !== nodeId && conn.to.nodeId !== nodeId
      );
      console.log('Connections after removal:', newConnections.length);
      return newConnections;
    });
    
    // Clear selection if this was the selected node
    if (selectedNode()?.id === nodeId) {
      setSelectedNode(null);
    }
    
    createMaterialFromNodes();
  };

  // Cached socket positions to reduce DOM queries during drag
  const [socketPositionCache, setSocketPositionCache] = createSignal(new Map());
  
  // Calculate socket position in screen coordinates (for SVG that's not transformed)
  const getSocketScreenPosition = (nodeId, socketId, socketType) => {
    const cacheKey = `${nodeId}-${socketId}-${socketType}`;
    const cache = socketPositionCache();
    
    // For dragged nodes, calculate position based on transform
    const isDraggedNodeSocket = draggedNode()?.id === nodeId;
    
    if (isDraggedNodeSocket) {
      // Get the socket element to use the same calculation method but with transform position
      const socketElement = document.querySelector(`[data-socket="${cacheKey}"]`);
      if (!socketElement || !nodeGraphRef) return null;
      
      const node = nodes().find(n => n.id === nodeId);
      if (!node) return null;
      
      const transform = draggedNodeTransform();
      
      // Get the current DOM position to calculate offset
      const socketRect = socketElement.getBoundingClientRect();
      const graphRect = nodeGraphRef.getBoundingClientRect();
      
      // Calculate how much the socket is offset from the node's top-left
      const nodeElement = socketElement.closest('.absolute');
      if (!nodeElement) return null;
      
      const nodeRect = nodeElement.getBoundingClientRect();
      const socketOffsetX = (socketRect.left + socketRect.width / 2) - nodeRect.left;
      const socketOffsetY = (socketRect.top + socketRect.height / 2) - nodeRect.top;
      
      // Apply the offset to the transform position
      const nodeScreenX = transform.x * zoom() + pan().x;
      const nodeScreenY = transform.y * zoom() + pan().y;
      
      const screenX = nodeScreenX + socketOffsetX;
      const screenY = nodeScreenY + socketOffsetY;
      
      return { x: screenX, y: screenY };
    }
    
    // Use cached position if available and not dragging this node
    if (cache.has(cacheKey)) {
      return cache.get(cacheKey);
    }
    
    const socketElement = document.querySelector(`[data-socket="${cacheKey}"]`);
    
    if (socketElement && nodeGraphRef) {
      const socketRect = socketElement.getBoundingClientRect();
      const graphRect = nodeGraphRef.getBoundingClientRect();
      
      // Calculate center of socket in screen coordinates relative to graph container
      const socketCenterX = socketRect.left + socketRect.width / 2 - graphRect.left;
      const socketCenterY = socketRect.top + socketRect.height / 2 - graphRect.top;
      
      const position = { x: socketCenterX, y: socketCenterY };
      
      // Cache the position
      const newCache = new Map(cache);
      newCache.set(cacheKey, position);
      setSocketPositionCache(newCache);
      
      return position;
    }
    
    return null;
  };

  // Calculate socket position in graph coordinates (for transformed elements)
  const getSocketPosition = (nodeId, socketId, socketType) => {
    // Try to get actual screen position first
    const screenPos = getSocketScreenPosition(nodeId, socketId, socketType);
    if (screenPos) {
      // Convert screen position to graph coordinates
      const graphX = (screenPos.x - pan().x) / zoom();
      const graphY = (screenPos.y - pan().y) / zoom();
      return { x: graphX, y: graphY };
    }
    
    // Fallback to calculated position if DOM element not found
    const node = nodes().find(n => n.id === nodeId);
    if (!node) return { x: 0, y: 0 };
    
    const nodeWidth = 200; // Approximate node width
    const nodeHeaderHeight = 48; // Header height
    const socketSpacing = 28; // Space between sockets
    const socketStartY = nodeHeaderHeight + 24; // Starting Y position for sockets
    
    let socketIndex = 0;
    
    if (socketType === 'input' && node.inputs) {
      socketIndex = node.inputs.findIndex(s => s.id === socketId);
    } else if (socketType === 'output' && node.outputs) {
      socketIndex = node.outputs.findIndex(s => s.id === socketId);
    }
    
    if (socketIndex === -1) return { x: 0, y: 0 };
    
    // Calculate socket position relative to node
    const socketX = socketType === 'input' ? node.position.x : node.position.x + nodeWidth;
    const socketY = node.position.y + socketStartY + (socketIndex * socketSpacing);
    
    return { x: socketX, y: socketY };
  };

  // Socket event handlers
  const handleSocketMouseDown = (e, nodeId, socket, socketType) => {
    e.preventDefault();
    e.stopPropagation();
    
    console.log('Socket clicked:', socketType, nodeId, socket.id);
    
    if (socketType === 'output') {
      console.log('Starting connection drag from output socket');
      
      // Get the socket position for logging
      const socketPos = getSocketPosition(nodeId, socket.id, socketType);
      console.log('Starting drag from socket position:', socketPos);
      
      setDraggingConnection({
        nodeId,
        socketId: socket.id,
        type: 'output'
      });
      
      const rect = nodeGraphRef.getBoundingClientRect();
      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;
      
      // Store screen coordinates for SVG rendering
      setDragConnectionEnd({ x: screenX, y: screenY });
    }
  };

  const handleSocketMouseEnter = (nodeId, socket, socketType) => {
    if (draggingConnection()) {
      const targetSocket = {
        nodeId,
        socketId: socket.id,
        type: socketType
      };
      
      // Only set as hovered if connection is valid
      if (canConnect(draggingConnection(), targetSocket)) {
        setHoveredSocket(targetSocket);
      }
    }
  };

  const handleSocketMouseLeave = () => {
    setHoveredSocket(null);
  };

  // Helper function for zooming towards a specific point
  const zoomToPoint = (zoomFactor, centerX, centerY) => {
    const oldZoom = zoom();
    const newZoom = Math.max(0.1, Math.min(3, oldZoom * zoomFactor));
    
    // Calculate the point in graph coordinates that we're zooming towards
    const currentPan = pan();
    const centerGraphX = (centerX - currentPan.x) / oldZoom;
    const centerGraphY = (centerY - currentPan.y) / oldZoom;
    
    // Calculate new pan to keep the same point at the center
    const newPan = {
      x: centerX - centerGraphX * newZoom,
      y: centerY - centerGraphY * newZoom
    };
    
    setZoom(newZoom);
    setPan(newPan);
  };

  // Handle zoom
  const handleWheel = (e) => {
    e.preventDefault();
    e.stopPropagation();
    
    const rect = nodeGraphRef.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;
    
    const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
    zoomToPoint(zoomFactor, mouseX, mouseY);
  };

  // Handle pan start and all-nodes drag start
  const handlePanStart = (e) => {
    if (e.button === 1 || (e.button === 0 && e.shiftKey)) { // Middle mouse or Shift+Left mouse for panning
      e.preventDefault();
      e.stopPropagation();
      setIsPanning(true);
      setPanStart({ x: e.clientX, y: e.clientY });
    } else if (e.button === 0 && !e.shiftKey && !e.altKey && !e.ctrlKey) { // Plain left mouse for dragging all nodes
      e.preventDefault();
      e.stopPropagation();
      setIsDraggingAllNodes(true);
      setAllNodesDragStart({ x: e.clientX, y: e.clientY });
    }
  };

  // Handle pan move
  const handlePanMove = (e) => {
    if (isPanning()) {
      e.preventDefault();
      const deltaX = e.clientX - panStart().x;
      const deltaY = e.clientY - panStart().y;
      
      setPan(prev => ({
        x: prev.x + deltaX / zoom(),
        y: prev.y + deltaY / zoom()
      }));
      
      setPanStart({ x: e.clientX, y: e.clientY });
    }
  };

  // Reset zoom and pan
  const resetView = () => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  };

  // Zoom to fit all nodes
  const zoomToFit = () => {
    const nodesList = nodes();
    if (nodesList.length === 0) return;
    
    const padding = 100;
    const minX = Math.min(...nodesList.map(n => n.position.x)) - padding;
    const maxX = Math.max(...nodesList.map(n => n.position.x + 200)) + padding; // Assume node width ~200
    const minY = Math.min(...nodesList.map(n => n.position.y)) - padding;
    const maxY = Math.max(...nodesList.map(n => n.position.y + 150)) + padding; // Assume node height ~150
    
    const width = maxX - minX;
    const height = maxY - minY;
    
    if (nodeGraphRef) {
      const rect = nodeGraphRef.getBoundingClientRect();
      const scaleX = rect.width / width;
      const scaleY = rect.height / height;
      const newZoom = Math.min(scaleX, scaleY, 1);
      
      const centerX = (minX + maxX) / 2;
      const centerY = (minY + maxY) / 2;
      
      setZoom(newZoom);
      setPan({
        x: rect.width / 2 - centerX * newZoom,
        y: rect.height / 2 - centerY * newZoom
      });
    }
  };

  // Handle asset drop
  const handleAssetDrop = (e) => {
    e.preventDefault();
    e.stopPropagation();
    
    // Try different data transfer formats
    let dragData = null;
    try {
      dragData = JSON.parse(e.dataTransfer.getData('application/json'));
    } catch {
      try {
        dragData = JSON.parse(e.dataTransfer.getData('text/plain'));
      } catch {
        try {
          dragData = JSON.parse(e.dataTransfer.getData('application/x-asset-drag'));
        } catch {
          // Try global drag data as fallback
          dragData = window._currentDragData;
        }
      }
    }
    
    if (!dragData) return;
    
    // Handle single asset or multiple assets
    const assets = dragData.assets || [dragData];
    
    for (const asset of assets) {
      console.log('Dragged asset:', {
        name: asset.name,
        extension: asset.extension,
        category: asset.category,
        mimeType: asset.mimeType
      });
      
      // Check if it's an HDR image for environment use
      const isHDR = asset.extension?.match(/\.(exr|hdr)$/i);
      
      // Check if it's a regular image asset
      const isImage = asset.category === 'images' || 
                     asset.extension?.match(/\.(jpg|jpeg|png|tiff|bmp|webp|gif|exr|hdr|dds|ktx)$/i) ||
                     asset.mimeType?.startsWith('image/') ||
                     // Check for HDR/texture formats that might not have image/ MIME type
                     asset.extension?.match(/\.(exr|hdr|dds|ktx)$/i);
      
      console.log('Is image?', isImage, 'Is HDR?', isHDR);
      
      console.log('Background type:', backgroundType(), 'Is HDR file:', isHDR);
      
      if (isHDR) {
        // Always allow HDR files to be set as background, auto-switch to HDR mode
        console.log('Setting HDR background and switching to HDR mode');
        setBackgroundType('hdr');
        setHdrBackground(asset);
        console.log('Set HDR background:', asset.name);
        break;
      } else if (isImage) {
        const rect = nodeGraphRef.getBoundingClientRect();
        const position = {
          x: e.clientX - rect.left - 100,
          y: e.clientY - rect.top - 50
        };
        
        // Create texture sample node with asset data
        addNode(NODE_TYPES.TEXTURE_SAMPLE, position, asset);
        
        console.log('Created texture node for:', asset.name);
        break; // Only create one node for now
      }
    }
  };

  // Handle drag over
  const handleDragOver = (e) => {
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = 'copy';
  };


  // Handle context menu
  const handleContextMenu = (e) => {
    e.preventDefault();
    e.stopPropagation();
    
    // First close any existing context menu
    setContextMenu(null);
    
    // Set position immediately
    const position = { x: e.clientX, y: e.clientY };
    console.log('Context menu position:', position);
    setContextMenuPosition(position);
    
    const contextMenuItems = [
      {
        label: 'Input Blocks',
        submenu: [
          {
            label: 'Basic Types',
            submenu: [
              {
                label: 'Float',
                action: () => addNode(NODE_TYPES.FLOAT, getCenterPosition())
              },
              {
                label: 'Vector2',
                action: () => addNode(NODE_TYPES.VECTOR2, getCenterPosition())
              },
              {
                label: 'Vector3',
                action: () => addNode(NODE_TYPES.VECTOR3, getCenterPosition())
              },
              {
                label: 'Vector4',
                action: () => addNode(NODE_TYPES.VECTOR4, getCenterPosition())
              },
              {
                label: 'Color3',
                action: () => addNode(NODE_TYPES.COLOR3, getCenterPosition())
              },
              {
                label: 'Color4',
                action: () => addNode(NODE_TYPES.COLOR4, getCenterPosition())
              }
            ]
          },
          {
            label: 'Textures',
            submenu: [
              {
                label: 'Texture',
                action: () => addNode(NODE_TYPES.TEXTURE, getCenterPosition())
              },
              {
                label: 'Reflection Texture',
                action: () => addNode(NODE_TYPES.REFLECTION_TEXTURE, getCenterPosition())
              },
              {
                label: 'Image Source',
                action: () => addNode(NODE_TYPES.IMAGE_SOURCE, getCenterPosition())
              }
            ]
          },
          {
            label: 'System',
            submenu: [
              {
                label: 'Time',
                action: () => addNode(NODE_TYPES.TIME, getCenterPosition())
              },
              {
                label: 'Delta Time',
                action: () => addNode(NODE_TYPES.DELTA_TIME, getCenterPosition())
              },
              {
                label: 'Screen Size',
                action: () => addNode(NODE_TYPES.SCREEN_SIZE, getCenterPosition())
              },
              {
                label: 'Frag Coord',
                action: () => addNode(NODE_TYPES.FRAG_COORD, getCenterPosition())
              },
              {
                label: 'Camera Position',
                action: () => addNode(NODE_TYPES.CAMERA_POSITION, getCenterPosition())
              },
              {
                label: 'Material Alpha',
                action: () => addNode(NODE_TYPES.MATERIAL_ALPHA, getCenterPosition())
              }
            ]
          },
          {
            label: 'Mesh Attributes',
            submenu: [
              {
                label: 'Mesh Position',
                action: () => addNode(NODE_TYPES.MESH_POSITION, getCenterPosition())
              },
              {
                label: 'Mesh Normal',
                action: () => addNode(NODE_TYPES.MESH_NORMAL, getCenterPosition())
              },
              {
                label: 'Mesh Tangent',
                action: () => addNode(NODE_TYPES.MESH_TANGENT, getCenterPosition())
              },
              {
                label: 'Mesh UV',
                action: () => addNode(NODE_TYPES.MESH_UV, getCenterPosition())
              },
              {
                label: 'Mesh Color',
                action: () => addNode(NODE_TYPES.MESH_COLOR, getCenterPosition())
              },
              {
                label: 'World Position',
                action: () => addNode(NODE_TYPES.WORLD_POSITION, getCenterPosition())
              },
              {
                label: 'World Normal',
                action: () => addNode(NODE_TYPES.WORLD_NORMAL, getCenterPosition())
              },
              {
                label: 'World Tangent',
                action: () => addNode(NODE_TYPES.WORLD_TANGENT, getCenterPosition())
              }
            ]
          }
        ]
      },
      {
        label: 'Output Blocks',
        submenu: [
          {
            label: 'Vertex Output',
            action: () => addNode(NODE_TYPES.VERTEX_OUTPUT, getCenterPosition())
          },
          {
            label: 'Fragment Output',
            action: () => addNode(NODE_TYPES.FRAGMENT_OUTPUT, getCenterPosition())
          },
          {
            label: 'Discard',
            action: () => addNode(NODE_TYPES.DISCARD, getCenterPosition())
          }
        ]
      },
      {
        label: 'Math Blocks',
        submenu: [
          {
            label: 'Basic Operations',
            submenu: [
              {
                label: 'Add',
                action: () => addNode(NODE_TYPES.ADD, getCenterPosition())
              },
              {
                label: 'Subtract',
                action: () => addNode(NODE_TYPES.SUBTRACT, getCenterPosition())
              },
              {
                label: 'Multiply',
                action: () => addNode(NODE_TYPES.MULTIPLY, getCenterPosition())
              },
              {
                label: 'Divide',
                action: () => addNode(NODE_TYPES.DIVIDE, getCenterPosition())
              },
              {
                label: 'Power',
                action: () => addNode(NODE_TYPES.POWER, getCenterPosition())
              },
              {
                label: 'Scale',
                action: () => addNode(NODE_TYPES.SCALE, getCenterPosition())
              },
              {
                label: 'Negate',
                action: () => addNode(NODE_TYPES.NEGATE, getCenterPosition())
              },
              {
                label: 'One Minus',
                action: () => addNode(NODE_TYPES.ONE_MINUS, getCenterPosition())
              },
              {
                label: 'Reciprocal',
                action: () => addNode(NODE_TYPES.RECIPROCAL, getCenterPosition())
              }
            ]
          },
          {
            label: 'Scientific',
            submenu: [
              {
                label: 'Absolute',
                action: () => addNode(NODE_TYPES.ABS, getCenterPosition())
              },
              {
                label: 'Sign',
                action: () => addNode(NODE_TYPES.SIGN, getCenterPosition())
              },
              {
                label: 'Floor',
                action: () => addNode(NODE_TYPES.FLOOR, getCenterPosition())
              },
              {
                label: 'Ceiling',
                action: () => addNode(NODE_TYPES.CEILING, getCenterPosition())
              },
              {
                label: 'Round',
                action: () => addNode(NODE_TYPES.ROUND, getCenterPosition())
              },
              {
                label: 'Fraction',
                action: () => addNode(NODE_TYPES.FRACT, getCenterPosition())
              },
              {
                label: 'Modulo',
                action: () => addNode(NODE_TYPES.MOD, getCenterPosition())
              },
              {
                label: 'Min',
                action: () => addNode(NODE_TYPES.MIN, getCenterPosition())
              },
              {
                label: 'Max',
                action: () => addNode(NODE_TYPES.MAX, getCenterPosition())
              },
              {
                label: 'Square Root',
                action: () => addNode(NODE_TYPES.SQRT, getCenterPosition())
              },
              {
                label: 'Exponential',
                action: () => addNode(NODE_TYPES.EXP, getCenterPosition())
              },
              {
                label: 'Logarithm',
                action: () => addNode(NODE_TYPES.LOG, getCenterPosition())
              }
            ]
          },
          {
            label: 'Trigonometry',
            submenu: [
              {
                label: 'Sine',
                action: () => addNode(NODE_TYPES.SIN, getCenterPosition())
              },
              {
                label: 'Cosine',
                action: () => addNode(NODE_TYPES.COS, getCenterPosition())
              },
              {
                label: 'Tangent',
                action: () => addNode(NODE_TYPES.TAN, getCenterPosition())
              },
              {
                label: 'ASin',
                action: () => addNode(NODE_TYPES.ASIN, getCenterPosition())
              },
              {
                label: 'ACos',
                action: () => addNode(NODE_TYPES.ACOS, getCenterPosition())
              },
              {
                label: 'ATan',
                action: () => addNode(NODE_TYPES.ATAN, getCenterPosition())
              },
              {
                label: 'ATan2',
                action: () => addNode(NODE_TYPES.ATAN2, getCenterPosition())
              },
              {
                label: 'Radians',
                action: () => addNode(NODE_TYPES.RADIANS, getCenterPosition())
              },
              {
                label: 'Degrees',
                action: () => addNode(NODE_TYPES.DEGREES, getCenterPosition())
              }
            ]
          },
          {
            label: 'Vector Math',
            submenu: [
              {
                label: 'Dot Product',
                action: () => addNode(NODE_TYPES.DOT, getCenterPosition())
              },
              {
                label: 'Cross Product',
                action: () => addNode(NODE_TYPES.CROSS, getCenterPosition())
              },
              {
                label: 'Normalize',
                action: () => addNode(NODE_TYPES.NORMALIZE, getCenterPosition())
              },
              {
                label: 'Length',
                action: () => addNode(NODE_TYPES.LENGTH, getCenterPosition())
              },
              {
                label: 'Distance',
                action: () => addNode(NODE_TYPES.DISTANCE, getCenterPosition())
              },
              {
                label: 'Reflect',
                action: () => addNode(NODE_TYPES.REFLECT, getCenterPosition())
              },
              {
                label: 'Refract',
                action: () => addNode(NODE_TYPES.REFRACT, getCenterPosition())
              },
              {
                label: 'Fresnel',
                action: () => addNode(NODE_TYPES.FRESNEL, getCenterPosition())
              },
              {
                label: 'Derivative',
                action: () => addNode(NODE_TYPES.DERIVATIVE, getCenterPosition())
              }
            ]
          },
          {
            label: 'Interpolation',
            submenu: [
              {
                label: 'Clamp',
                action: () => addNode(NODE_TYPES.CLAMP, getCenterPosition())
              },
              {
                label: 'Saturate',
                action: () => addNode(NODE_TYPES.SATURATE, getCenterPosition())
              },
              {
                label: 'Remap',
                action: () => addNode(NODE_TYPES.REMAP, getCenterPosition())
              },
              {
                label: 'Lerp',
                action: () => addNode(NODE_TYPES.LERP, getCenterPosition())
              },
              {
                label: 'NLerp',
                action: () => addNode(NODE_TYPES.NLERP, getCenterPosition())
              },
              {
                label: 'Step',
                action: () => addNode(NODE_TYPES.STEP, getCenterPosition())
              },
              {
                label: 'SmoothStep',
                action: () => addNode(NODE_TYPES.SMOOTHSTEP, getCenterPosition())
              }
            ]
          }
        ]
      },
      {
        label: 'Logical Blocks',
        submenu: [
          {
            label: 'And',
            action: () => addNode(NODE_TYPES.AND, getCenterPosition())
          },
          {
            label: 'Or',
            action: () => addNode(NODE_TYPES.OR, getCenterPosition())
          },
          {
            label: 'Xor',
            action: () => addNode(NODE_TYPES.XOR, getCenterPosition())
          },
          {
            label: 'Not',
            action: () => addNode(NODE_TYPES.NOT, getCenterPosition())
          },
          {
            label: 'Equal',
            action: () => addNode(NODE_TYPES.EQUAL, getCenterPosition())
          },
          {
            label: 'Not Equal',
            action: () => addNode(NODE_TYPES.NOT_EQUAL, getCenterPosition())
          },
          {
            label: 'Greater Than',
            action: () => addNode(NODE_TYPES.GREATER_THAN, getCenterPosition())
          },
          {
            label: 'Greater Or Equal',
            action: () => addNode(NODE_TYPES.GREATER_OR_EQUAL, getCenterPosition())
          },
          {
            label: 'Less Than',
            action: () => addNode(NODE_TYPES.LESS_THAN, getCenterPosition())
          },
          {
            label: 'Less Or Equal',
            action: () => addNode(NODE_TYPES.LESS_OR_EQUAL, getCenterPosition())
          }
        ]
      },
      {
        label: 'Conversion Blocks',
        submenu: [
          {
            label: 'Color Merger',
            action: () => addNode(NODE_TYPES.COLOR_MERGER, getCenterPosition())
          },
          {
            label: 'Color Splitter',
            action: () => addNode(NODE_TYPES.COLOR_SPLITTER, getCenterPosition())
          },
          {
            label: 'Vector Merger',
            action: () => addNode(NODE_TYPES.VECTOR_MERGER, getCenterPosition())
          },
          {
            label: 'Vector Splitter',
            action: () => addNode(NODE_TYPES.VECTOR_SPLITTER, getCenterPosition())
          }
        ]
      },
      {
        label: 'Noise Blocks',
        submenu: [
          {
            label: 'Simplex Perlin 3D',
            action: () => addNode(NODE_TYPES.SIMPLEX_PERLIN_3D, getCenterPosition())
          },
          {
            label: 'Voronoi Noise',
            action: () => addNode(NODE_TYPES.VORONOI_NOISE, getCenterPosition())
          },
          {
            label: 'Worley Noise 3D',
            action: () => addNode(NODE_TYPES.WORLEY_NOISE_3D, getCenterPosition())
          },
          {
            label: 'Cloud',
            action: () => addNode(NODE_TYPES.CLOUD, getCenterPosition())
          },
          {
            label: 'Random Number',
            action: () => addNode(NODE_TYPES.RANDOM_NUMBER, getCenterPosition())
          }
        ]
      },
      {
        label: 'Matrix Blocks',
        submenu: [
          {
            label: 'World Matrix',
            action: () => addNode(NODE_TYPES.WORLD_MATRIX, getCenterPosition())
          },
          {
            label: 'View Matrix',
            action: () => addNode(NODE_TYPES.VIEW_MATRIX, getCenterPosition())
          },
          {
            label: 'Projection Matrix',
            action: () => addNode(NODE_TYPES.PROJECTION_MATRIX, getCenterPosition())
          },
          {
            label: 'World View Matrix',
            action: () => addNode(NODE_TYPES.WORLD_VIEW_MATRIX, getCenterPosition())
          },
          {
            label: 'World View Projection Matrix',
            action: () => addNode(NODE_TYPES.WORLD_VIEW_PROJECTION_MATRIX, getCenterPosition())
          },
          {
            label: 'Matrix Builder',
            action: () => addNode(NODE_TYPES.MATRIX_BUILDER, getCenterPosition())
          }
        ]
      },
      {
        label: 'PBR Blocks',
        submenu: [
          {
            label: 'PBR Metallic Roughness',
            action: () => addNode(NODE_TYPES.PBR_METALLIC_ROUGHNESS, getCenterPosition())
          },
          {
            label: 'Anisotropy',
            action: () => addNode(NODE_TYPES.ANISOTROPY, getCenterPosition())
          },
          {
            label: 'Clear Coat',
            action: () => addNode(NODE_TYPES.CLEARCOAT, getCenterPosition())
          },
          {
            label: 'Iridescence',
            action: () => addNode(NODE_TYPES.IRIDESCENCE, getCenterPosition())
          },
          {
            label: 'Sheen',
            action: () => addNode(NODE_TYPES.SHEEN, getCenterPosition())
          },
          {
            label: 'Sub Surface',
            action: () => addNode(NODE_TYPES.SUB_SURFACE, getCenterPosition())
          }
        ]
      },
      {
        label: 'Texture Operations',
        submenu: [
          {
            label: 'TriPlanar',
            action: () => addNode(NODE_TYPES.TRIPLANAR, getCenterPosition())
          },
          {
            label: 'BiPlanar',
            action: () => addNode(NODE_TYPES.BIPLANAR, getCenterPosition())
          },
          {
            label: 'Normal Map',
            action: () => addNode(NODE_TYPES.NORMAL_MAP, getCenterPosition())
          },
          {
            label: 'Normal Blend',
            action: () => addNode(NODE_TYPES.NORMAL_BLEND, getCenterPosition())
          },
          {
            label: 'Parallax',
            action: () => addNode(NODE_TYPES.PARALLAX, getCenterPosition())
          },
          {
            label: 'Twirl',
            action: () => addNode(NODE_TYPES.TWIRL, getCenterPosition())
          }
        ]
      },
      {
        label: 'Advanced Blocks',
        submenu: [
          {
            label: 'Transform',
            action: () => addNode(NODE_TYPES.TRANSFORM, getCenterPosition())
          },
          {
            label: 'Bones',
            action: () => addNode(NODE_TYPES.BONES, getCenterPosition())
          },
          {
            label: 'Morph Targets',
            action: () => addNode(NODE_TYPES.MORPH_TARGETS, getCenterPosition())
          },
          {
            label: 'Fog',
            action: () => addNode(NODE_TYPES.FOG, getCenterPosition())
          },
          {
            label: 'Light Information',
            action: () => addNode(NODE_TYPES.LIGHT_INFORMATION, getCenterPosition())
          },
          {
            label: 'Scene Depth',
            action: () => addNode(NODE_TYPES.SCENE_DEPTH, getCenterPosition())
          },
          {
            label: 'View Direction',
            action: () => addNode(NODE_TYPES.VIEW_DIRECTION, getCenterPosition())
          },
          {
            label: 'Loop',
            action: () => addNode(NODE_TYPES.LOOP, getCenterPosition())
          },
          {
            label: 'Storage Read',
            action: () => addNode(NODE_TYPES.STORAGE_READ, getCenterPosition())
          },
          {
            label: 'Storage Write',
            action: () => addNode(NODE_TYPES.STORAGE_WRITE, getCenterPosition())
          }
        ]
      },
      {
        label: 'Particle Blocks',
        submenu: [
          {
            label: 'Particle Color',
            action: () => addNode(NODE_TYPES.PARTICLE_COLOR, getCenterPosition())
          },
          {
            label: 'Particle Texture',
            action: () => addNode(NODE_TYPES.PARTICLE_TEXTURE, getCenterPosition())
          },
          {
            label: 'Particle UV',
            action: () => addNode(NODE_TYPES.PARTICLE_UV, getCenterPosition())
          },
          {
            label: 'Particle Position',
            action: () => addNode(NODE_TYPES.PARTICLE_POSITION, getCenterPosition())
          },
          {
            label: 'Particle Ramp Gradient',
            action: () => addNode(NODE_TYPES.PARTICLE_RAMP_GRADIENT, getCenterPosition())
          }
        ]
      },
      {
        label: 'Vector',
        submenu: [
          {
            label: 'Dot Product',
            action: () => addNode(NODE_TYPES.DOT_PRODUCT, getCenterPosition())
          },
          {
            label: 'Cross Product',
            action: () => addNode(NODE_TYPES.CROSS_PRODUCT, getCenterPosition())
          },
          {
            label: 'Distance',
            action: () => addNode(NODE_TYPES.DISTANCE, getCenterPosition())
          },
          {
            label: 'Length',
            action: () => addNode(NODE_TYPES.LENGTH, getCenterPosition())
          },
          {
            label: 'Reflect',
            action: () => addNode(NODE_TYPES.REFLECT, getCenterPosition())
          },
          {
            label: 'Refract',
            action: () => addNode(NODE_TYPES.REFRACT, getCenterPosition())
          },
          {
            label: 'Normal Map',
            action: () => addNode(NODE_TYPES.NORMAL_MAP, getCenterPosition())
          }
        ]
      },
      {
        label: 'Procedural',
        submenu: [
          {
            label: 'Noise',
            action: () => addNode(NODE_TYPES.NOISE, getCenterPosition())
          },
          {
            label: 'Voronoi Noise',
            action: () => addNode(NODE_TYPES.VORONOI_NOISE, getCenterPosition())
          },
          {
            label: 'Wave',
            action: () => addNode(NODE_TYPES.WAVE, getCenterPosition())
          },
          {
            label: 'Gradient',
            action: () => addNode(NODE_TYPES.GRADIENT, getCenterPosition())
          }
        ]
      },
      {
        label: 'Image Processing',
        submenu: [
          {
            label: 'Desaturate',
            action: () => addNode(NODE_TYPES.DESATURATE, getCenterPosition())
          },
          {
            label: 'Hue',
            action: () => addNode(NODE_TYPES.HUE, getCenterPosition())
          },
          {
            label: 'Saturation',
            action: () => addNode(NODE_TYPES.SATURATION, getCenterPosition())
          },
          {
            label: 'Contrast',
            action: () => addNode(NODE_TYPES.CONTRAST, getCenterPosition())
          },
          {
            label: 'Brightness',
            action: () => addNode(NODE_TYPES.BRIGHTNESS, getCenterPosition())
          },
          {
            label: 'Invert',
            action: () => addNode(NODE_TYPES.INVERT, getCenterPosition())
          },
          {
            label: 'Posterize',
            action: () => addNode(NODE_TYPES.POSTERIZE, getCenterPosition())
          }
        ]
      },
      {
        label: 'Utilities',
        submenu: [
          {
            label: 'Fresnel',
            action: () => addNode(NODE_TYPES.FRESNEL, getCenterPosition())
          },
          {
            label: 'Mask',
            action: () => addNode(NODE_TYPES.MASK, getCenterPosition())
          },
          {
            label: 'Mix',
            action: () => addNode(NODE_TYPES.MIX, getCenterPosition())
          },
          {
            label: 'Conditional',
            action: () => addNode(NODE_TYPES.CONDITIONAL, getCenterPosition())
          }
        ]
      },
      {
        label: 'Materials',
        submenu: [
          {
            label: 'PBR Metallic Roughness',
            action: () => addNode(NODE_TYPES.PBR_METALLIC_ROUGHNESS, getCenterPosition())
          }
        ]
      },
      { separator: true },
      {
        label: 'Reset View',
        action: resetView
      },
      {
        label: 'Zoom to Fit',
        action: zoomToFit
      }
    ];

    // Use setTimeout to ensure position is set before showing menu
    setTimeout(() => {
      setContextMenu(contextMenuItems);
    }, 0);
  };

  // Effects
  createEffect(() => {
    updatePreviewMesh();
  });

  createEffect(() => {
    createMaterialFromNodes();
  });

  // Update mesh when preview shape changes
  createEffect(() => {
    previewShape(); // Access the signal to make this reactive
    if (previewScene) {
      updatePreviewMesh();
    }
  });

  // Clear socket cache when zoom or pan changes
  createEffect(() => {
    zoom(); // Access zoom signal to make this reactive
    pan(); // Access pan signal to make this reactive
    setSocketPositionCache(new Map()); // Clear cache on zoom/pan changes
  });

  // Update camera when distance changes
  createEffect(() => {
    cameraDistance();
    updateCameraDistance();
  });

  // Update lighting when controls change
  createEffect(() => {
    lightIntensity();
    ambientIntensity();
    updateLighting();
  });

  // Update shadows when controls change
  createEffect(() => {
    shadowsEnabled();
    updateShadows();
  });

  // Update wireframe when controls change
  createEffect(() => {
    wireframeMode();
    updateWireframe();
  });

  // Update ground visibility when controls change
  createEffect(() => {
    groundVisible();
    updateGroundVisibility();
  });

  // Update mesh scale when controls change
  createEffect(() => {
    meshScale();
    updateMeshScale();
  });

  // Update auto rotation when controls change
  createEffect(() => {
    autoRotate();
    updateAutoRotation();
  });

  // Update background when controls change
  createEffect(() => {
    backgroundColor();
    backgroundType();
    hdrBackground();
    updateBackground();
  });

  // Recreate material when PBR setting changes
  createEffect(() => {
    usePBR();
    createMaterialFromNodes();
  });

  onMount(() => {
    // Hide bottom and right panels when materials viewport mounts
    editorActions.setScenePanelOpen(false);
    editorActions.setAssetPanelOpen(false);
    
    setTimeout(() => {
      initPreviewScene();
      initializeDefaultNodes();
    }, 100);
    
    // Add global mouse event listeners
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  });

  onCleanup(() => {
    document.removeEventListener('mousemove', handleMouseMove);
    document.removeEventListener('mouseup', handleMouseUp);
    
    // Clean up auto rotation
    if (autoRotationAnimation) {
      clearInterval(autoRotationAnimation);
    }
    
    if (previewEngine) {
      previewEngine.dispose();
    }
    
    // Clean up backdrop meshes and shadows
    if (groundMesh) {
      groundMesh.dispose();
    }
    if (backdropMesh) {
      backdropMesh.dispose();
    }
    if (shadowGenerator) {
      shadowGenerator.dispose();
    }
  });

  return (
    <div class="h-full flex bg-base-100">
      {/* Left Panel - Preview */}
      <div class="w-80 border-r border-base-300 flex flex-col bg-base-200">
        {/* Preview Canvas - Fixed at top */}
        <div class="h-80 bg-base-300 relative border-b border-base-300">
          <canvas
            ref={previewCanvasRef}
            class="w-full h-full"
            style={{ display: 'block', 'object-fit': 'contain' }}
          />
          
          {/* Shape Selector Overlay - Bottom Left */}
          <div class="absolute bottom-2 left-2 flex gap-2">
            <button
              class={`btn btn-sm ${previewShape() === 'sphere' ? 'btn-primary' : 'btn-ghost'} bg-opacity-80 backdrop-blur-sm`}
              onClick={() => setPreviewShape('sphere')}
              title="Sphere"
            >
              <IconSphere class="w-4 h-4" />
            </button>
            <button
              class={`btn btn-sm ${previewShape() === 'cube' ? 'btn-primary' : 'btn-ghost'} bg-opacity-80 backdrop-blur-sm`}
              onClick={() => setPreviewShape('cube')}
              title="Cube"
            >
              <IconCube class="w-4 h-4" />
            </button>
            <button
              class={`btn btn-sm ${previewShape() === 'plane' ? 'btn-primary' : 'btn-ghost'} bg-opacity-80 backdrop-blur-sm`}
              onClick={() => setPreviewShape('plane')}
              title="Plane"
            >
              <IconSquare class="w-4 h-4" />
            </button>
            <button
              class={`btn btn-sm ${previewShape() === 'cylinder' ? 'btn-primary' : 'btn-ghost'} bg-opacity-80 backdrop-blur-sm`}
              onClick={() => setPreviewShape('cylinder')}
              title="Cylinder"
            >
              <IconCircle class="w-4 h-4" />
            </button>
            <button
              class={`btn btn-sm ${previewShape() === 'torus' ? 'btn-primary' : 'btn-ghost'} bg-opacity-80 backdrop-blur-sm`}
              onClick={() => setPreviewShape('torus')}
              title="Torus"
            >
              <IconHexagon class="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Preview Controls */}
        <div 
          class="flex-1 overflow-y-auto border-b border-base-300 p-2 space-y-2"
          onDrop={handleAssetDrop}
          onDragOver={handleDragOver}
        >
          {/* Material Section */}
          <div class="bg-base-100 border-base-300 border rounded-lg">
            <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().material ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
              <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('material')}>
                <IconSettings class="w-3 h-3" />
                Material
              </div>
              <input
                type="checkbox"
                checked={sectionsOpen().material}
                onChange={(e) => {
                  e.stopPropagation();
                  toggleSection('material');
                }}
                onClick={(e) => e.stopPropagation()}
                class="toggle toggle-primary toggle-xs"
              />
            </div>
            <Show when={sectionsOpen().material}>
              <div class="!p-2">
              {/* Material Type */}
              <div class="form-control">
                <div class="flex items-center justify-between">
                  <label class="label-text text-sm font-medium">Material Type</label>
                  <div class="flex items-center gap-2">
                    <span class="text-xs text-base-content/60">Standard</span>
                    <input
                      type="checkbox"
                      class="toggle toggle-xs toggle-primary"
                      checked={usePBR()}
                      onChange={(e) => setUsePBR(e.target.checked)}
                    />
                    <span class="text-xs text-base-content/60">PBR</span>
                  </div>
                </div>
              </div>
              </div>
            </Show>
          </div>

          {/* Camera Section */}
          <div class="bg-base-100 border-base-300 border rounded-lg">
            <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().camera ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
              <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('camera')}>
                <IconVideo class="w-3 h-3" />
                Camera
              </div>
              <input
                type="checkbox"
                checked={sectionsOpen().camera}
                onChange={(e) => {
                  e.stopPropagation();
                  toggleSection('camera');
                }}
                onClick={(e) => e.stopPropagation()}
                class="toggle toggle-primary toggle-xs"
              />
            </div>
            <Show when={sectionsOpen().camera}>
              <div class="!p-2">
              {/* Camera Controls */}
              <div class="form-control">
                <div class="flex items-center justify-between">
                  <button
                    class="btn btn-xs btn-outline btn-primary"
                    onClick={() => {
                      if (previewCamera) {
                        previewCamera.alpha = Math.PI / 4;
                        previewCamera.beta = Math.PI / 3;
                        previewCamera.radius = 6;
                        setCameraDistance(6);
                      }
                    }}
                    title="Reset Camera"
                  >
                    <IconSettings class="w-3 h-3" />
                    Reset
                  </button>
                  <div class="text-xs text-base-content/60 bg-base-200 px-2 py-1 rounded">
                    Distance: {Math.round((previewCamera?.radius || cameraDistance()) * 10) / 10}
                  </div>
                </div>
              </div>
              </div>
            </Show>
          </div>

          {/* Lighting Section */}
          <div class="bg-base-100 border-base-300 border rounded-lg">
            <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().lighting ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
              <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('lighting')}>
                <IconBulb class="w-3 h-3" />
                Lighting
              </div>
              <input
                type="checkbox"
                checked={sectionsOpen().lighting}
                onChange={(e) => {
                  e.stopPropagation();
                  toggleSection('lighting');
                }}
                onClick={(e) => e.stopPropagation()}
                class="toggle toggle-primary toggle-xs"
              />
            </div>
            <Show when={sectionsOpen().lighting}>
              <div class="!p-2">
              {/* Lighting Controls */}
              <div class="form-control">
                <div class="space-y-3">
                  <div class="flex items-center justify-between">
                    <span class="text-xs text-base-content/80">Directional</span>
                    <div class="flex items-center gap-2">
                      <input
                        type="range"
                        min="0"
                        max="2"
                        step="0.1"
                        value={lightIntensity()}
                        class="range range-xs range-primary w-20"
                        onChange={(e) => setLightIntensity(parseFloat(e.target.value))}
                      />
                      <span class="text-xs text-base-content/60 bg-base-200 px-2 py-1 rounded w-10 text-center">{lightIntensity().toFixed(1)}</span>
                    </div>
                  </div>
                  <div class="flex items-center justify-between">
                    <span class="text-xs text-base-content/80">Ambient</span>
                    <div class="flex items-center gap-2">
                      <input
                        type="range"
                        min="0"
                        max="1"
                        step="0.1"
                        value={ambientIntensity()}
                        class="range range-xs range-primary w-20"
                        onChange={(e) => setAmbientIntensity(parseFloat(e.target.value))}
                      />
                      <span class="text-xs text-base-content/60 bg-base-200 px-2 py-1 rounded w-10 text-center">{ambientIntensity().toFixed(1)}</span>
                    </div>
                  </div>
                  <div class="flex items-center justify-between">
                    <span class="text-xs text-base-content/80">Shadows</span>
                    <input
                      type="checkbox"
                      class="toggle toggle-xs toggle-primary"
                      checked={shadowsEnabled()}
                      onChange={(e) => setShadowsEnabled(e.target.checked)}
                    />
                  </div>
                </div>
              </div>
              </div>
            </Show>
          </div>

          {/* Environment Section */}
          <div class="bg-base-100 border-base-300 border rounded-lg">
            <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().environment ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
              <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('environment')}>
                <IconWorld class="w-3 h-3" />
                Environment
              </div>
              <input
                type="checkbox"
                checked={sectionsOpen().environment}
                onChange={(e) => {
                  e.stopPropagation();
                  toggleSection('environment');
                }}
                onClick={(e) => e.stopPropagation()}
                class="toggle toggle-primary toggle-xs"
              />
            </div>
            <Show when={sectionsOpen().environment}>
              <div class="!p-2">
              {/* Environment Controls */}
              <div class="form-control">
                <div class="space-y-3">
                  <div class="flex items-center justify-between">
                    <span class="text-xs text-base-content/80">Type</span>
                    <div class="btn-group btn-group-xs">
                      <button
                        class={`btn btn-xs ${backgroundType() === 'color' ? 'btn-primary' : 'btn-outline'}`}
                        onClick={() => setBackgroundType('color')}
                      >
                        Color
                      </button>
                      <button
                        class={`btn btn-xs ${backgroundType() === 'hdr' ? 'btn-primary' : 'btn-outline'}`}
                        onClick={() => setBackgroundType('hdr')}
                      >
                        HDR
                      </button>
                    </div>
                  </div>
                  
                  {/* Color Background */}
                  <Show when={backgroundType() === 'color'}>
                    <div class="flex items-center justify-between">
                      <span class="text-xs text-base-content/80">Color</span>
                      <input
                        type="color"
                        value={backgroundColor()}
                        class="input input-xs w-10 h-8 p-0 rounded border border-base-300 cursor-pointer"
                        onChange={(e) => setBackgroundColor(e.target.value)}
                      />
                    </div>
                  </Show>
                  
                  {/* HDR Background */}
                  <Show when={backgroundType() === 'hdr'}>
                    <div class="space-y-2">
                      <div class="flex items-center justify-between">
                        <span class="text-xs text-base-content/80">HDR Image</span>
                        <div class="btn-group btn-group-xs">
                          <button
                            class="btn btn-xs btn-outline btn-primary"
                            onClick={() => document.getElementById('hdr-file-input').click()}
                            title="Upload HDR file"
                          >
                            <IconPhoto class="w-3 h-3" />
                          </button>
                          <button
                            class="btn btn-xs btn-outline btn-error"
                            onClick={() => setHdrBackground(null)}
                            disabled={!hdrBackground()}
                            title="Clear HDR"
                          >
                            <IconX class="w-3 h-3" />
                          </button>
                        </div>
                      </div>
                      
                      {/* Hidden file input */}
                      <input
                        id="hdr-file-input"
                        type="file"
                        accept=".hdr,.exr,.dds,.ktx"
                        style={{ display: 'none' }}
                        onChange={handleHDRFileUpload}
                      />
                      
                      <Show when={hdrBackground()}>
                        <div class="alert alert-info py-2">
                          <span class="text-xs">{hdrBackground().name}</span>
                        </div>
                      </Show>
                      <Show when={!hdrBackground()}>
                        <div class="alert py-3">
                          <div class="text-xs text-center">
                            <div>Click 📷 to upload HDR/EXR file</div>
                            <div>or drag from assets</div>
                          </div>
                        </div>
                      </Show>
                    </div>
                  </Show>
                </div>
              </div>
              </div>
            </Show>
          </div>

          {/* Preview Section */}
          <div class="bg-base-100 border-base-300 border rounded-lg">
            <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().preview ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
              <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('preview')}>
                <IconWorld class="w-3 h-3" />
                Preview
              </div>
              <input
                type="checkbox"
                checked={sectionsOpen().preview}
                onChange={(e) => {
                  e.stopPropagation();
                  toggleSection('preview');
                }}
                onClick={(e) => e.stopPropagation()}
                class="checkbox checkbox-xs checkbox-primary"
              />
            </div>
            <Show when={sectionsOpen().preview}>
              <div class="!p-2">
                <div class="form-control">
                  <div class="space-y-3">
                    
                    {/* Wireframe Mode */}
                    <div class="flex items-center justify-between">
                      <span class="text-xs text-base-content/80">Wireframe</span>
                      <input
                        type="checkbox"
                        class="toggle toggle-xs toggle-primary"
                        checked={wireframeMode()}
                        onChange={(e) => setWireframeMode(e.target.checked)}
                      />
                    </div>

                    {/* Auto Rotation */}
                    <div class="flex items-center justify-between">
                      <span class="text-xs text-base-content/80">Auto Rotate</span>
                      <input
                        type="checkbox"
                        class="toggle toggle-xs toggle-primary"
                        checked={autoRotate()}
                        onChange={(e) => setAutoRotate(e.target.checked)}
                      />
                    </div>

                    {/* Ground Visibility */}
                    <div class="flex items-center justify-between">
                      <span class="text-xs text-base-content/80">Show Ground</span>
                      <input
                        type="checkbox"
                        class="toggle toggle-xs toggle-primary"
                        checked={groundVisible()}
                        onChange={(e) => setGroundVisible(e.target.checked)}
                      />
                    </div>

                    {/* Mesh Scale */}
                    <div class="flex items-center justify-between">
                      <span class="text-xs text-base-content/80">Scale</span>
                      <div class="flex items-center gap-2">
                        <input
                          type="range"
                          min="0.5"
                          max="3.0"
                          step="0.1"
                          value={meshScale()}
                          class="range range-xs range-primary w-16"
                          onChange={(e) => setMeshScale(parseFloat(e.target.value))}
                        />
                        <span class="text-xs text-base-content/60 w-8">{meshScale().toFixed(1)}</span>
                      </div>
                    </div>

                    {/* Camera Reset Button */}
                    <div class="flex items-center justify-between">
                      <span class="text-xs text-base-content/80">Camera</span>
                      <button
                        class="btn btn-xs btn-outline btn-primary"
                        onClick={resetCamera}
                        title="Reset camera to default position"
                      >
                        Reset
                      </button>
                    </div>
                    
                  </div>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </div>
      
      {/* Right Panel - Node Graph */}
      <div class="flex-1 flex flex-col">
        {/* Node Graph Canvas */}
        <div 
          ref={nodeGraphRef}
          class={`flex-1 relative bg-base-100 h-full ${
            isDraggingAllNodes() ? 'cursor-grabbing' : 
            isPanning() ? 'cursor-grabbing' : 
            'cursor-grab'
          }`}
          style={{
            'background-image': 'radial-gradient(circle, #374151 1px, transparent 1px)',
            'background-size': `${20 * zoom()}px ${20 * zoom()}px`,
            'background-position': `${pan().x}px ${pan().y}px`
          }}
          onDrop={handleAssetDrop}
          onDragOver={handleDragOver}
          onWheel={handleWheel}
          onMouseDown={handlePanStart}
          onContextMenu={handleContextMenu}
        >
          
          {/* Nodes Container */}
          <div 
            class="absolute inset-0"
            style={{
              transform: `translate(${pan().x}px, ${pan().y}px) scale(${zoom()})`,
              'transform-origin': '0 0'
            }}
          >
            {/* Nodes */}
            <For each={nodes()}>
              {(node) => {
                const isDragged = () => draggedNode()?.id === node.id;
                const position = () => isDragged() ? draggedNodeTransform() : node.position;
                
                return (
                  <div
                    class={`absolute bg-base-200 border shadow-lg min-w-44 rounded-lg overflow-hidden ${
                      selectedNode() === node ? 'border-primary ring-2 ring-primary/30' : 'border-base-300 hover:border-base-content/20'
                    } ${isDragged() ? '' : 'transition-all duration-200'}`}
                    style={{
                      left: `${position().x}px`,
                      top: `${position().y}px`,
                      'z-index': getNodeZIndex(node.id)
                    }}
                  >
                {/* Node Header - Compact like Unreal */}
                <div 
                  class={`px-3 py-1.5 text-sm font-medium cursor-grab select-none transition-all flex items-center gap-2 ${
                    selectedNode() === node ? 'text-primary bg-primary/20' : 'text-base-content/80 hover:text-base-content bg-base-300 hover:bg-base-300/80'
                  }`}
                  onMouseDown={(e) => handleNodeMouseDown(e, node)}
                >
                  {/* Type indicator */}
                  <div class={`w-2 h-2 rounded-sm ${
                    node.type === 'MaterialOutput' ? 'bg-success' :
                    node.type === 'TextureSample' ? 'bg-info' :
                    node.type === 'Constant' ? 'bg-warning' :
                    'bg-neutral'
                  }`}></div>
                  <span class="truncate flex-1">{node.title}</span>
                  
                  {/* Delete button - only show for non-output nodes */}
                  <Show when={node.id !== 'material-output'}>
                    <button
                      class="w-4 h-4 flex items-center justify-center rounded hover:bg-error/20 hover:text-error transition-colors relative z-10"
                      style="pointer-events: auto;"
                      onMouseDown={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        console.log('Delete button mousedown');
                      }}
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        console.log('Delete button clicked for node:', node.id);
                        removeNode(node.id);
                      }}
                      onPointerDown={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                      }}
                      title="Delete Node"
                    >
                      <IconX class="w-3 h-3" />
                    </button>
                  </Show>
                  
                  {/* Active indicator line like tabs */}
                  <Show when={selectedNode() === node}>
                    <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-primary"></div>
                  </Show>
                </div>
                
                {/* Node Body - Compact like Unreal */}
                <div class="bg-base-100 p-2 space-y-1">
                  {/* Texture Preview - Compact */}
                  <Show when={node.type === NODE_TYPES.TEXTURE_SAMPLE && node.asset}>
                    <TexturePreview asset={node.asset} />
                  </Show>
                  
                  {/* Inputs - Compact */}
                  <Show when={node.inputs?.length > 0}>
                    <For each={node.inputs}>
                      {(input) => (
                        <div class="flex items-center justify-between py-0.5 group">
                          <div class="flex items-center gap-1.5 flex-1 min-w-0">
                            <div 
                              class={`w-2.5 h-2.5 rounded-full border cursor-pointer transition-all duration-200 pointer-events-auto flex-shrink-0 ${
                                hoveredSocket()?.nodeId === node.id && hoveredSocket()?.socketId === input.id
                                  ? 'border-primary scale-110'
                                  : 'border-base-400 hover:border-primary'
                              }`}
                              style={{
                                'background-color': 
                                  input.type === 'color' ? '#ff6b6b' :
                                  input.type === 'float' ? '#51cf66' :
                                  input.type === 'vector' ? '#339af0' :
                                  input.type === 'texture' ? '#ffd43b' :
                                  '#6c757d',
                                position: 'relative',
                                'z-index': '10'
                              }}
                              data-socket={`${node.id}-${input.id}-input`}
                              onMouseDown={(e) => handleSocketMouseDown(e, node.id, input, 'input')}
                              onMouseEnter={() => handleSocketMouseEnter(node.id, input, 'input')}
                              onMouseLeave={handleSocketMouseLeave}
                              onClick={(e) => {
                                e.stopPropagation();
                              }}
                            ></div>
                            <span class="text-xs text-base-content/80 truncate">{input.name}</span>
                          </div>
                          <Show when={input.type === 'float' && input.value !== null}>
                            <input
                              type="number"
                              class="input input-xs w-14 text-right text-xs border-base-300 bg-base-200"
                              value={input.value}
                              step="0.1"
                              onChange={(e) => {
                                const newNodes = nodes().map(n => 
                                  n.id === node.id 
                                    ? {
                                        ...n,
                                        inputs: n.inputs.map(i =>
                                          i.id === input.id 
                                            ? { ...i, value: parseFloat(e.target.value) }
                                            : i
                                        )
                                      }
                                    : n
                                );
                                setNodes(newNodes);
                                createMaterialFromNodes();
                              }}
                            />
                          </Show>
                          <Show when={input.type === 'color' && input.value !== null && input.value instanceof Color3}>
                            <input
                              type="color"
                              class="w-8 h-6 rounded border border-base-300 cursor-pointer"
                              value={`#${Math.round(input.value.r * 255).toString(16).padStart(2, '0')}${Math.round(input.value.g * 255).toString(16).padStart(2, '0')}${Math.round(input.value.b * 255).toString(16).padStart(2, '0')}`}
                              onChange={(e) => {
                                const hex = e.target.value;
                                const r = parseInt(hex.slice(1, 3), 16) / 255;
                                const g = parseInt(hex.slice(3, 5), 16) / 255;
                                const b = parseInt(hex.slice(5, 7), 16) / 255;
                                const color = new Color3(r, g, b);
                                
                                const newNodes = nodes().map(n => 
                                  n.id === node.id 
                                    ? {
                                        ...n,
                                        inputs: n.inputs.map(i =>
                                          i.id === input.id 
                                            ? { ...i, value: color }
                                            : i
                                        )
                                      }
                                    : n
                                );
                                setNodes(newNodes);
                                createMaterialFromNodes();
                              }}
                            />
                          </Show>
                        </div>
                      )}
                    </For>
                  </Show>
                  
                  {/* Outputs - Compact */}
                  <Show when={node.outputs?.length > 0}>
                    <For each={node.outputs}>
                      {(output) => (
                        <div class="flex items-center justify-between py-0.5 group">
                          <div class="flex items-center gap-1.5 flex-1 min-w-0">
                            <span class="text-xs text-base-content/80 truncate">{output.name}</span>
                          </div>
                          <div 
                            class={`w-3 h-3 rounded-full border cursor-pointer transition-all duration-200 pointer-events-auto flex-shrink-0 ${
                              hoveredSocket()?.nodeId === node.id && hoveredSocket()?.socketId === output.id
                                ? 'border-primary scale-125 shadow-lg shadow-primary/50'
                                : 'border-base-400 hover:border-primary hover:scale-110'
                            }`}
                            style={{
                              'background-color': 
                                output.type === 'color' ? '#ff6b6b' :
                                output.type === 'float' ? '#51cf66' :
                                output.type === 'vector' ? '#339af0' :
                                output.type === 'texture' ? '#ffd43b' :
                                '#6c757d',
                              position: 'relative',
                              'z-index': '10'
                            }}
                            data-socket={`${node.id}-${output.id}-output`}
                            onMouseDown={(e) => handleSocketMouseDown(e, node.id, output, 'output')}
                            onMouseEnter={() => handleSocketMouseEnter(node.id, output, 'output')}
                            onMouseLeave={handleSocketMouseLeave}
                            onClick={(e) => {
                              e.stopPropagation();
                            }}
                          ></div>
                        </div>
                      )}
                    </For>
                  </Show>
                </div>
              </div>
                );
              }}
            </For>
          </div>
          

          {/* Connection Lines SVG */}
          <svg 
            class="absolute inset-0 w-full h-full"
            style={{
              overflow: 'visible',
              'pointer-events': 'none'
            }}
          >
            {/* Existing Connections */}
            <For each={connections()}>
              {(connection) => {
                return (
                  <ConnectionLine
                    connection={connection}
                    nodes={nodes()}
                    getSocketScreenPosition={getSocketScreenPosition}
                    draggedNodeId={draggedNode()?.id}
                    draggedNodeTransform={draggedNodeTransform}
                    zoom={zoom}
                    pan={pan}
                    onRemove={removeConnection}
                  />
                );
              }}
            </For>
            
            {/* Dragging Connection */}
            <Show when={draggingConnection()}>
              {() => {
                const fromPos = getSocketScreenPosition(draggingConnection().nodeId, draggingConnection().socketId, 'output');
                if (!fromPos) return null;
                
                const fromX = fromPos.x;
                const fromY = fromPos.y;
                const toX = dragConnectionEnd().x;
                const toY = dragConnectionEnd().y;
                
                const controlOffset = Math.max(80, Math.abs(toX - fromX) * 0.4);
                const pathData = `M ${fromX} ${fromY} C ${fromX + controlOffset} ${fromY} ${toX - controlOffset} ${toY} ${toX} ${toY}`;
                
                return (
                  <path
                    d={pathData}
                    stroke="#60a5fa"
                    stroke-width="3"
                    fill="none"
                    stroke-dasharray="5,5"
                    class="opacity-80"
                  />
                );
              }}
            </Show>
          </svg>
          
          {/* Context Menu */}
          <Show when={contextMenu() && contextMenuPosition()}>
            <ContextMenu
              items={contextMenu()}
              position={contextMenuPosition()}
              onClose={() => {
                setContextMenu(null);
                setContextMenuPosition(null);
              }}
            />
          </Show>
          
          {/* Control Buttons Overlay - Top Left */}
          <div class="absolute top-4 left-4 flex gap-2">
            {/* Zoom Controls */}
            <div class="flex items-center gap-1 bg-base-100/90 backdrop-blur-sm rounded px-2 py-1 border border-base-300 shadow-lg">
              <button 
                class="btn btn-xs btn-ghost"
                onClick={() => {
                  if (nodeGraphRef) {
                    const rect = nodeGraphRef.getBoundingClientRect();
                    zoomToPoint(0.8, rect.width / 2, rect.height / 2);
                  }
                }}
                title="Zoom Out"
              >
                <IconMinus class="w-3 h-3" />
              </button>
              <span class="text-xs font-mono w-12 text-center">{Math.round(zoom() * 100)}%</span>
              <button 
                class="btn btn-xs btn-ghost"
                onClick={() => {
                  if (nodeGraphRef) {
                    const rect = nodeGraphRef.getBoundingClientRect();
                    zoomToPoint(1.25, rect.width / 2, rect.height / 2);
                  }
                }}
                title="Zoom In"
              >
                <IconPlus class="w-3 h-3" />
              </button>
              <div class="divider divider-horizontal mx-1"></div>
              <button 
                class="btn btn-xs btn-ghost"
                onClick={resetView}
                title="Reset View"
              >
                <IconSettings class="w-3 h-3" />
              </button>
              <button 
                class="btn btn-xs btn-ghost"
                onClick={zoomToFit}
                title="Zoom to Fit"
              >
                <IconSphere class="w-3 h-3" />
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}