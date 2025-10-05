import { createSignal, createEffect, onMount, onCleanup, Show, For, createMemo } from 'solid-js';
import { renderStore } from '@/render/store';
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
  IconTexture
} from '@tabler/icons-solidjs';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial.js';
import { PBRMaterial } from '@babylonjs/core/Materials/PBR/pbrMaterial.js';
import { NodeMaterial } from '@babylonjs/core/Materials/Node/nodeMaterial.js';
import { Color3 } from '@babylonjs/core/Maths/math.color.js';

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
import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';
import { FreeCamera } from '@babylonjs/core/Cameras/freeCamera.js';
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight.js';
import { DirectionalLight } from '@babylonjs/core/Lights/directionalLight.js';
import { ShadowGenerator } from '@babylonjs/core/Lights/Shadows/shadowGenerator.js';
import { Scene } from '@babylonjs/core/scene.js';
import { Engine } from '@babylonjs/core/Engines/engine.js';
import { Texture } from '@babylonjs/core/Materials/Textures/texture.js';
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
  
  // Node types
  const NODE_TYPES = {
    MATERIAL_OUTPUT: 'MaterialOutput',
    TEXTURE_SAMPLE: 'TextureSample',
    CONSTANT: 'Constant',
    MULTIPLY: 'Multiply',
    ADD: 'Add',
    SUBTRACT: 'Subtract',
    DIVIDE: 'Divide',
    LERP: 'Lerp',
    FRESNEL: 'Fresnel',
    CLAMP: 'Clamp',
    POWER: 'Power',
    COLOR: 'Color',
    TIME: 'Time',
    UV_COORDINATES: 'UVCoordinates',
    NORMAL_MAP: 'NormalMap',
    DOT_PRODUCT: 'DotProduct',
    CROSS_PRODUCT: 'CrossProduct',
    NOISE: 'Noise',
    GRADIENT: 'Gradient',
    MASK: 'Mask',
    MIX: 'Mix',
    SATURATE: 'Saturate',
    ABS: 'Abs',
    FLOOR: 'Floor',
    CEIL: 'Ceil',
    FRACT: 'Fract',
    SIN: 'Sin',
    COS: 'Cos'
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
    
    // Setup camera with side angle view
    const camera = new FreeCamera('previewCamera', new Vector3(3, 2, -3), previewScene);
    camera.setTarget(new Vector3(0, -0.5, 0)); // Look at the object position
    camera.attachControl(previewCanvasRef, true);
    
    // Setup lighting with shadows
    // Ambient lighting
    const ambientLight = new HemisphericLight('ambientLight', new Vector3(0, 1, 0), previewScene);
    ambientLight.intensity = 0.4;
    ambientLight.diffuse = new Color3(1, 1, 1);
    
    // Directional light for shadows
    const directionalLight = new DirectionalLight('dirLight', new Vector3(-1, -1, -1), previewScene);
    directionalLight.position = new Vector3(3, 5, 3);
    directionalLight.intensity = 0.8;
    directionalLight.diffuse = new Color3(1, 1, 1);
    
    // Shadow generator
    shadowGenerator = new ShadowGenerator(1024, directionalLight);
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
          <pattern id="checker" width="20" height="20" patternUnits="userSpaceOnUse">
            <rect x="0" y="0" width="10" height="10" fill="#f0f0f0"/>
            <rect x="10" y="10" width="10" height="10" fill="#f0f0f0"/>
            <rect x="10" y="0" width="10" height="10" fill="#ffffff"/>
            <rect x="0" y="10" width="10" height="10" fill="#ffffff"/>
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill="url(#checker)"/>
        <!-- Grid lines overlay -->
        <defs>
          <pattern id="gridlines" width="20" height="20" patternUnits="userSpaceOnUse">
            <path d="M 20 0 L 0 0 0 20" fill="none" stroke="#d0d0d0" stroke-width="0.5"/>
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill="url(#gridlines)"/>
      </svg>
    `), previewScene);
    
    checkerTexture.uScale = 15;
    checkerTexture.vScale = 15;
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
    if (!asset) return null;
    
    // Construct URL for the actual full-resolution image file
    if (asset.path && asset.path.includes('projects/')) {
      // Asset path already includes full project path
      const cleanPath = asset.path.startsWith('/') ? asset.path.slice(1) : asset.path;
      return `http://localhost:3001/file/${cleanPath}`;
    } else if (asset.name || asset.id) {
      // Construct full path - based on thumbnail pattern we see in console
      const fileName = asset.name || asset.id;
      return `http://localhost:3001/file/projects/test/assets/textures/${fileName}`;
    } else if (asset.path) {
      // Try to prepend the project path to the asset path
      const cleanPath = asset.path.startsWith('/') ? asset.path.slice(1) : asset.path;
      return `http://localhost:3001/file/projects/test/${cleanPath}`;
    }
    return null;
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

  // Create material from node graph - proper NodeMaterial implementation
  const createMaterialFromNodes = () => {
    const scene = previewScene;
    if (!scene) return;
    
    // Use StandardMaterial for simpler, more predictable results
    const material = new StandardMaterial('NodeBasedMaterial', scene);
    
    // Set some defaults that will be visible
    material.diffuseColor = new Color3(1.0, 0.0, 1.0); // Bright magenta default
    material.specularColor = new Color3(0.2, 0.2, 0.2); // Low specular
    
    console.log('Default material created with gray color');
    
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
              material.diffuseColor = colorInput.value;
              console.log('Applied base color:', colorInput.value);
            }
          } else if (sourceNode.type === NODE_TYPES.TEXTURE_SAMPLE) {
            // Handle texture connection to base color
            const textureInput = sourceNode.inputs.find(i => i.id === 'texture');
            if (textureInput?.value) {
              const asset = textureInput.value;
              const texture = createTextureFromAsset(asset, scene);
              if (texture) {
                material.diffuseTexture = texture;
                
                // Special handling for HDR textures on base color
                const extension = asset.extension?.toLowerCase() || asset.name?.split('.').pop()?.toLowerCase();
                if (extension === 'exr' || extension === 'hdr') {
                  // For HDR textures on base color, ensure material doesn't become transparent
                  material.alpha = 1.0;
                  material.transparencyMode = null;
                  
                  // Set a neutral diffuse color to modulate the HDR texture
                  material.diffuseColor = new Color3(0.5, 0.5, 0.5);
                  console.log('Applied HDR texture to base color with exposure compensation');
                } else {
                  // Reset diffuse color to white for regular textures
                  material.diffuseColor = new Color3(1.0, 1.0, 1.0);
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
                // For StandardMaterial, we can use specularTexture for roughness
                material.specularTexture = texture;
                console.log('Applied roughness texture');
              }
            }
          } else if (sourceNode.type === NODE_TYPES.CONSTANT) {
            const valueInput = sourceNode.inputs.find(i => i.id === 'value');
            if (valueInput?.value !== undefined) {
              // For StandardMaterial, we use specularPower (inverse relationship)
              material.specularPower = Math.max(1, (1 - valueInput.value) * 128);
              console.log('Applied roughness (specularPower):', material.specularPower);
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
                material.reflectionTexture = texture;
                console.log('Applied metallic texture');
              }
            }
          } else if (sourceNode.type === NODE_TYPES.CONSTANT) {
            const valueInput = sourceNode.inputs.find(i => i.id === 'value');
            if (valueInput?.value !== undefined) {
              const metallic = valueInput.value;
              material.specularColor = new Color3(metallic, metallic, metallic);
              console.log('Applied metallic (specular):', metallic);
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
    }
    
    if (newNode) {
      setNodes(prev => [...prev, newNode]);
    }
  };

  // Handle node drag
  const handleNodeMouseDown = (e, node) => {
    e.preventDefault();
    e.stopPropagation();
    
    const rect = nodeGraphRef.getBoundingClientRect();
    // Convert screen coordinates to node graph coordinates
    const screenX = e.clientX - rect.left;
    const screenY = e.clientY - rect.top;
    const graphX = (screenX - pan().x) / zoom();
    const graphY = (screenY - pan().y) / zoom();
    
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
      
      // Check if it's an image asset - include HDR formats like EXR and HDR
      const isImage = asset.category === 'images' || 
                     asset.extension?.match(/\.(jpg|jpeg|png|tiff|bmp|webp|gif|exr|hdr|dds|ktx)$/i) ||
                     asset.mimeType?.startsWith('image/') ||
                     // Check for HDR/texture formats that might not have image/ MIME type
                     asset.extension?.match(/\.(exr|hdr|dds|ktx)$/i);
      
      console.log('Is image?', isImage);
      
      if (isImage) {
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
        label: 'Inputs',
        submenu: [
          {
            label: 'Texture Sample',
            action: () => addNode(NODE_TYPES.TEXTURE_SAMPLE, getCenterPosition())
          },
          {
            label: 'Constant',
            action: () => addNode(NODE_TYPES.CONSTANT, getCenterPosition())
          },
          {
            label: 'Color',
            action: () => addNode(NODE_TYPES.COLOR, getCenterPosition())
          },
          {
            label: 'Time',
            action: () => addNode(NODE_TYPES.TIME, getCenterPosition())
          },
          {
            label: 'UV Coordinates',
            action: () => addNode(NODE_TYPES.UV_COORDINATES, getCenterPosition())
          }
        ]
      },
      {
        label: 'Textures',
        submenu: [
          {
            label: 'Diffuse/Albedo',
            action: () => addNode(NODE_TYPES.TEXTURE_SAMPLE, getCenterPosition())
          },
          {
            label: 'Normal Map',
            action: () => addNode(NODE_TYPES.NORMAL_MAP, getCenterPosition())
          },
          {
            label: 'Roughness Map',
            action: () => addNode(NODE_TYPES.TEXTURE_SAMPLE, getCenterPosition())
          },
          {
            label: 'Metallic Map',
            action: () => addNode(NODE_TYPES.TEXTURE_SAMPLE, getCenterPosition())
          },
          {
            label: 'Ambient Occlusion',
            action: () => addNode(NODE_TYPES.TEXTURE_SAMPLE, getCenterPosition())
          },
          {
            label: 'Emissive Map',
            action: () => addNode(NODE_TYPES.TEXTURE_SAMPLE, getCenterPosition())
          },
          { separator: true },
          {
            label: 'HDR Exposure Control',
            action: () => addNode(NODE_TYPES.MULTIPLY, getCenterPosition())
          }
        ]
      },
      {
        label: 'Math',
        submenu: [
          {
            label: 'Basic',
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
              }
            ]
          },
          {
            label: 'Functions',
            submenu: [
              {
                label: 'Power',
                action: () => addNode(NODE_TYPES.POWER, getCenterPosition())
              },
              {
                label: 'Absolute',
                action: () => addNode(NODE_TYPES.ABS, getCenterPosition())
              },
              {
                label: 'Sine',
                action: () => addNode(NODE_TYPES.SIN, getCenterPosition())
              },
              {
                label: 'Cosine',
                action: () => addNode(NODE_TYPES.COS, getCenterPosition())
              }
            ]
          },
          {
            label: 'Utility',
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
                label: 'Lerp',
                action: () => addNode(NODE_TYPES.LERP, getCenterPosition())
              }
            ]
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
          }
        ]
      },
      {
        label: 'Advanced',
        submenu: [
          {
            label: 'Fresnel',
            action: () => addNode(NODE_TYPES.FRESNEL, getCenterPosition())
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

  onMount(() => {
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
      <div class="w-96 border-r border-base-300 flex flex-col bg-base-200">
        {/* Preview Controls */}
        <div class="p-4 border-b border-base-300">
          <h3 class="text-md font-semibold mb-3">Material Preview</h3>
          
          {/* Preview Shape Selector */}
          <div class="flex gap-2 mb-3">
            <button
              class={`btn btn-sm ${previewShape() === 'sphere' ? 'btn-primary' : 'btn-ghost'}`}
              onClick={() => setPreviewShape('sphere')}
              title="Sphere"
            >
              <IconSphere class="w-4 h-4" />
            </button>
            <button
              class={`btn btn-sm ${previewShape() === 'cube' ? 'btn-primary' : 'btn-ghost'}`}
              onClick={() => setPreviewShape('cube')}
              title="Cube"
            >
              <IconCube class="w-4 h-4" />
            </button>
          </div>
        </div>
        
        {/* Preview Canvas */}
        <div class="h-64 bg-base-300 relative">
          <canvas
            ref={previewCanvasRef}
            class="w-full h-full"
            style={{ display: 'block' }}
          />
        </div>
        
        {/* Material Info */}
        <div class="p-4 border-t border-base-300">
          <div class="text-sm text-base-content/60">
            <div class="flex items-center gap-2 mb-2">
              <IconPalette class="w-4 h-4" />
              <span class="font-medium">Node Material</span>
            </div>
            <div class="text-xs space-y-1">
              <div>Nodes: {nodes().length} | Connections: {connections().length}</div>
              <div class={`flex items-center gap-1 ${currentMaterial() ? 'text-success' : 'text-warning'}`}>
                <div class={`w-2 h-2 rounded-full ${currentMaterial() ? 'bg-success' : 'bg-warning'}`}></div>
                {currentMaterial() ? 'Material Built' : 'Building...'}
              </div>
              <Show when={currentMaterial()?.name}>
                <div class="text-xs text-base-content/40">
                  {currentMaterial().name}
                </div>
              </Show>
            </div>
          </div>
        </div>
      </div>
      
      {/* Right Panel - Node Graph */}
      <div class="flex-1 flex flex-col">
        {/* Node Graph Header */}
        <div class="p-4 border-b border-base-300 bg-base-200">
          <div class="flex items-center justify-between">
            <h3 class="text-md font-semibold">Material Graph</h3>
            <div class="flex gap-2">
              {/* Zoom Controls */}
              <div class="flex items-center gap-1 bg-base-100 rounded px-2 py-1 border border-base-300">
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
        
        {/* Node Graph Canvas */}
        <div 
          ref={nodeGraphRef}
          class={`flex-1 relative bg-base-100 ${
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
                      'z-index': isDragged() ? '1000' : 'auto'
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
          
          {/* Debug Info */}
          <div class="absolute top-2 left-2 bg-black/80 text-white p-2 rounded text-xs font-mono pointer-events-none">
            <div>Nodes: {nodes().length}</div>
            <div>Connections: {connections().length}</div>
            <div>Material: {currentMaterial() ? 'Created' : 'None'}</div>
            <div>Color Nodes: {nodes().filter(n => n.type === NODE_TYPES.COLOR).length}</div>
            <Show when={nodes().find(n => n.type === NODE_TYPES.COLOR)}>
              <div>First Color: {nodes().find(n => n.type === NODE_TYPES.COLOR)?.inputs.find(i => i.id === 'color')?.value ? 'Set' : 'Null'}</div>
            </Show>
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
        </div>
      </div>
    </div>
  );
}