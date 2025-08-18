import { createEffect, onCleanup } from 'solid-js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { TransformNode } from '@babylonjs/core/Meshes/transformNode';
import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder';
import { Color3 } from '@babylonjs/core/Maths/math.color';
import '@babylonjs/core/Meshes/Builders/linesBuilder';
import { editorStore } from '@/layout/stores/EditorStore';
import { viewportStore } from '@/layout/stores/ViewportStore';

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
    console.log('🔳 Grid: createGrid called', { scene: !!scene, gridSettings, showGrid: viewport().showGrid });
    if (!scene || !gridSettings.enabled || !viewport().showGrid) {
      console.log('🔳 Grid: Skipping grid creation - missing requirements');
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
        regularGrid.color = Color3.FromHexString(gridSettings.cellColor || '#334155');
      }
      
      if (sectionLines.length > 0) {
        const maxLinesPerSystem = isWebGPU ? 50 : sectionLines.length;
        const linesToCreate = sectionLines.slice(0, maxLinesPerSystem);
        const sectionGrid = MeshBuilder.CreateLineSystem("__grid_sections__", { lines: linesToCreate }, scene);
        sectionGrid.parent = gridContainer;
        sectionGrid.isPickable = false;
        sectionGrid.material.alpha = 0.25;
        sectionGrid.color = Color3.FromHexString(gridSettings.sectionColor || '#475569');
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
        regularGrid.color = Color3.FromHexString(gridSettings.cellColor || '#334155');
      }
      
      if (sectionLines.length > 0) {
        const maxLinesPerSystem = isWebGPU ? 50 : sectionLines.length;
        const linesToCreate = sectionLines.slice(0, maxLinesPerSystem);
        const sectionGrid = MeshBuilder.CreateLineSystem("__grid_sections__", { lines: linesToCreate }, scene);
        sectionGrid.parent = gridContainer;
        sectionGrid.isPickable = false;
        sectionGrid.material.alpha = 0.25;
        sectionGrid.color = Color3.FromHexString(gridSettings.sectionColor || '#475569');
      }
    }
    
    gridContainer.isPickable = false;
    gridContainer._isSystemObject = true;
    gridContainer.position = new Vector3(
      gridSettings.position[0],
      gridSettings.position[1], 
      gridSettings.position[2]
    );

    console.log('🔳 Grid: Grid created successfully!', { 
      position: gridContainer.position, 
      childCount: gridContainer.getChildMeshes().length,
      infinite: gridSettings.infiniteGrid
    });

    return gridContainer;
  };

  const updateGrid = () => {
    const scene = sceneSignal();
    if (!scene) {
      console.log('🔳 Grid: No scene available for updateGrid');
      return;
    }

    console.log('🔳 Grid: updateGrid called', { 
      hasScene: !!scene, 
      gridEnabled: settings().grid.enabled, 
      showGrid: viewport().showGrid,
      gridSettings: settings().grid
    });

    if (gridRef) {
      console.log('🔳 Grid: Disposing existing grid');
      gridRef.dispose();
      gridRef = null;
    }

    if (settings().grid.enabled && viewport().showGrid) {
      console.log('🔳 Grid: Creating new grid...');
      createGrid(scene, settings().grid).then(grid => {
        gridRef = grid;
        console.log('🔳 Grid: Grid assigned to gridRef', !!grid);
      });
    } else {
      console.log('🔳 Grid: Grid creation skipped - disabled or hidden');
    }
  };

  createEffect(() => {
    // Track scene signal, grid settings, and viewport showGrid
    const scene = sceneSignal();
    const gridSettings = settings().grid;
    const showGrid = viewport().showGrid;
    
    console.log('🔳 Grid: createEffect triggered', { hasScene: !!scene, gridEnabled: gridSettings.enabled, showGrid });
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