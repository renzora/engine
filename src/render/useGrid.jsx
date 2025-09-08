import { createEffect, onCleanup } from 'solid-js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { TransformNode } from '@babylonjs/core/Meshes/transformNode';
import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder';
import { Color3 } from '@babylonjs/core/Maths/math.color';
import '@babylonjs/core/Meshes/Builders/linesBuilder';
import { editorStore } from '@/layout/stores/EditorStore';
import { viewportStore } from '@/layout/stores/ViewportStore';

// Helper function to convert OKLCH to RGB using CSS Color Module 4 conversion
const oklchToRgb = (l, c, h) => {
  // Convert OKLCH to OKLAB
  const hRad = h * Math.PI / 180;
  const a = c * Math.cos(hRad);
  const b = c * Math.sin(hRad);
  
  // Convert OKLAB to linear RGB using matrices from CSS Color Module 4 spec
  const l_ = l + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = l - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = l - 0.0894841775 * a - 1.2914855480 * b;
  
  const l3 = l_ * l_ * l_;
  const m3 = m_ * m_ * m_;
  const s3 = s_ * s_ * s_;
  
  let r = +4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3;
  let g = -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3;
  let bl = -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3;
  
  // Gamma correction for sRGB
  r = r > 0.0031308 ? 1.055 * Math.pow(r, 1/2.4) - 0.055 : 12.92 * r;
  g = g > 0.0031308 ? 1.055 * Math.pow(g, 1/2.4) - 0.055 : 12.92 * g;
  bl = bl > 0.0031308 ? 1.055 * Math.pow(bl, 1/2.4) - 0.055 : 12.92 * bl;
  
  return {
    r: Math.max(0, Math.min(1, r)),
    g: Math.max(0, Math.min(1, g)),
    b: Math.max(0, Math.min(1, bl))
  };
};

// Helper function to parse color string and convert to RGB
const parseColorToRgb = (colorStr) => {
  if (colorStr.startsWith('oklch(')) {
    const match = colorStr.match(/oklch\(([\d.%]+)\s+([\d.]+)\s+([\d.]+)\)/);
    if (match) {
      let l = parseFloat(match[1]);
      const c = parseFloat(match[2]);
      const h = parseFloat(match[3]);
      
      // Convert percentage lightness to decimal
      if (match[1].includes('%')) {
        l = l / 100;
      }
      
      return oklchToRgb(l, c, h);
    }
  }
  
  if (colorStr.startsWith('rgb(')) {
    const match = colorStr.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
    if (match) {
      return {
        r: parseInt(match[1]) / 255,
        g: parseInt(match[2]) / 255,
        b: parseInt(match[3]) / 255
      };
    }
  }
  
  if (colorStr.startsWith('#')) {
    const hex = colorStr.slice(1);
    return {
      r: parseInt(hex.slice(0, 2), 16) / 255,
      g: parseInt(hex.slice(2, 4), 16) / 255,
      b: parseInt(hex.slice(4, 6), 16) / 255
    };
  }
  
  return null;
};

// Helper function to get DaisyUI color from CSS custom properties
const getDaisyUIColor = (colorName) => {
  const style = getComputedStyle(document.documentElement);
  // Map short names to actual DaisyUI CSS custom property names
  const colorPropertyMap = {
    'p': 'color-primary',
    's': 'color-secondary', 
    'a': 'color-accent',
    'b1': 'color-base-100',
    'b2': 'color-base-200',
    'b3': 'color-base-300',
    'bc': 'color-base-content',
    'n': 'color-neutral'
  };
  
  const propertyName = colorPropertyMap[colorName] || colorName;
  const colorValue = style.getPropertyValue(`--${propertyName}`).trim();
  
  if (colorValue) {
    const rgb = parseColorToRgb(colorValue);
    if (rgb) {
      return new Color3(rgb.r, rgb.g, rgb.b);
    }
  }
  
  // Fallback colors that match common DaisyUI themes
  switch (colorName) {
    case 'p': return new Color3(0.235, 0.506, 0.957); // primary blue
    case 's': return new Color3(0.545, 0.365, 0.957); // secondary purple
    case 'a': return new Color3(0.024, 0.714, 0.831); // accent cyan
    case 'b1': return new Color3(0.067, 0.094, 0.149); // base-100 dark
    case 'b2': return new Color3(0.122, 0.161, 0.216); // base-200
    case 'b3': return new Color3(0.220, 0.255, 0.318); // base-300
    case 'bc': return new Color3(0.9, 0.9, 0.9); // base-content light
    default: return new Color3(0.235, 0.506, 0.957); // fallback to primary
  }
};

export function useGrid(sceneSignal) {
  const settings = () => editorStore.settings;
  const viewport = () => viewportStore;
  let gridRef = null;
  
  const getUnitScale = (unit) => {
    const scales = {
      'meters': 1.0,
      'centimeters': 0.01,
      'millimeters': 0.001,
      'feet': 0.3048,
      'inches': 0.0254
    };
    return scales[unit] || 1.0;
  };

  const createGrid = async (scene, gridSettings) => {
    // Create grid mesh system
    if (!scene || !gridSettings.enabled || !viewport().showGrid) {
      return;
    }

    const unitScale = getUnitScale(gridSettings.unit);
    const cellSize = gridSettings.cellSize * unitScale;
    const sectionSize = gridSettings.sectionSize || 10;
    const isWebGPU = scene.getEngine().constructor.name === 'WebGPUEngine';
    
    let gridContainer;
    
    if (gridSettings.infiniteGrid) {
      const gridSize = isWebGPU ? 200 : 1000;
      const gridCells = Math.floor(gridSize / cellSize);
      const maxCells = isWebGPU ? 50 : 500;
      const actualGridCells = Math.min(gridCells, maxCells);
      const regularLines = [];
      const sectionLines = [];
      const halfSize = gridSize / 2;
      
      for (let i = -actualGridCells; i <= actualGridCells; i++) {
        const x = i * cellSize;
        if (Math.abs(x) <= halfSize) {
          const line = [
            new Vector3(x, 0, -halfSize),
            new Vector3(x, 0, halfSize)
          ];
          
          if (i % sectionSize === 0) {
            sectionLines.push(line);
          } else {
            regularLines.push(line);
          }
        }
      }
      
      for (let i = -actualGridCells; i <= actualGridCells; i++) {
        const z = i * cellSize;
        if (Math.abs(z) <= halfSize) {
          const line = [
            new Vector3(-halfSize, 0, z),
            new Vector3(halfSize, 0, z)
          ];
          
          if (i % sectionSize === 0) {
            sectionLines.push(line);
          } else {
            regularLines.push(line);
          }
        }
      }
      
      gridContainer = new TransformNode("__grid_container__", scene);
      
      if (regularLines.length > 0) {
        const maxLinesPerSystem = isWebGPU ? 100 : regularLines.length;
        const linesToCreate = regularLines.slice(0, maxLinesPerSystem);
        
        const regularGrid = MeshBuilder.CreateLineSystem("__grid_regular__", { lines: linesToCreate }, scene);
        regularGrid.parent = gridContainer;
        regularGrid.isPickable = false;
        regularGrid.material.alpha = 0.15;
        regularGrid.color = getDaisyUIColor('bc').scale(0.3); // Use theme border color, dimmed
      }
      
      if (sectionLines.length > 0) {
        const maxLinesPerSystem = isWebGPU ? 50 : sectionLines.length;
        const linesToCreate = sectionLines.slice(0, maxLinesPerSystem);
        const sectionGrid = MeshBuilder.CreateLineSystem("__grid_sections__", { lines: linesToCreate }, scene);
        sectionGrid.parent = gridContainer;
        sectionGrid.isPickable = false;
        sectionGrid.material.alpha = 0.25;
        sectionGrid.color = getDaisyUIColor('bc').scale(0.5); // Use theme border color, brighter for sections
      }
    } else {
      const gridSize = gridSettings.size * unitScale;
      const gridCells = Math.floor(gridSize / cellSize);
      const regularLines = [];
      const sectionLines = [];
      const halfSize = gridSize / 2;
      
      for (let i = 0; i <= gridCells; i++) {
        const x = (i / gridCells) * gridSize - halfSize;
        const line = [
          new Vector3(x, 0, -halfSize),
          new Vector3(x, 0, halfSize)
        ];
        
        if (i % sectionSize === 0 || i === gridCells) {
          sectionLines.push(line);
        } else {
          regularLines.push(line);
        }
      }
      
      for (let i = 0; i <= gridCells; i++) {
        const z = (i / gridCells) * gridSize - halfSize;
        const line = [
          new Vector3(-halfSize, 0, z),
          new Vector3(halfSize, 0, z)
        ];
        
        if (i % sectionSize === 0 || i === gridCells) {
          sectionLines.push(line);
        } else {
          regularLines.push(line);
        }
      }
      
      gridContainer = new TransformNode("__grid_container__", scene);
      
      if (regularLines.length > 0) {
        const maxLinesPerSystem = isWebGPU ? 100 : regularLines.length;
        const linesToCreate = regularLines.slice(0, maxLinesPerSystem);
        
        const regularGrid = MeshBuilder.CreateLineSystem("__grid_regular__", { lines: linesToCreate }, scene);
        regularGrid.parent = gridContainer;
        regularGrid.isPickable = false;
        regularGrid.material.alpha = 0.15;
        regularGrid.color = getDaisyUIColor('bc').scale(0.3); // Use theme border color, dimmed
      }
      
      if (sectionLines.length > 0) {
        const maxLinesPerSystem = isWebGPU ? 50 : sectionLines.length;
        const linesToCreate = sectionLines.slice(0, maxLinesPerSystem);
        const sectionGrid = MeshBuilder.CreateLineSystem("__grid_sections__", { lines: linesToCreate }, scene);
        sectionGrid.parent = gridContainer;
        sectionGrid.isPickable = false;
        sectionGrid.material.alpha = 0.25;
        sectionGrid.color = getDaisyUIColor('bc').scale(0.5); // Use theme border color, brighter for sections
      }
    }
    
    gridContainer.isPickable = false;
    gridContainer._isSystemObject = true;
    gridContainer.position = new Vector3(
      gridSettings.position[0],
      gridSettings.position[1], 
      gridSettings.position[2]
    );

    // Grid created successfully
    return gridContainer;
  };

  const updateGrid = () => {
    const scene = sceneSignal();
    if (!scene) {
      return;
    }

    // Update grid based on current settings

    if (gridRef) {
      // Dispose existing grid
      gridRef.dispose();
      gridRef = null;
    }

    if (settings().grid.enabled && viewport().showGrid) {
      // Create new grid
      createGrid(scene, settings().grid).then(grid => {
        gridRef = grid;
      });
    }
  };

  createEffect(() => {
    // Track scene signal, grid settings, and viewport showGrid
    const scene = sceneSignal();
    const gridSettings = settings().grid;
    const showGrid = viewport().showGrid;
    
    // Grid settings or scene changed, update grid
    updateGrid();
  });

  onCleanup(() => {
    if (gridRef) {
      gridRef.dispose();
      gridRef = null;
    }
  });

  return null;
}