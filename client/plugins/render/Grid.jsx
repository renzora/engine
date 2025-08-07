import { useEffect, useRef } from 'react';
import { useSnapshot } from 'valtio';
import { globalStore } from '@/store.js';
import * as BABYLON from '@babylonjs/core';

export function useGrid(scene) {
  const settings = useSnapshot(globalStore.editor.settings);
  const viewport = useSnapshot(globalStore.editor.viewport);
  const gridRef = useRef(null);

  // Helper function to get unit scale factor (converts to meters)
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

  const createGrid = (scene, gridSettings) => {
    if (!scene || !gridSettings.enabled || !viewport.showGrid) return;

    const unitScale = getUnitScale(gridSettings.unit);
    const cellSize = gridSettings.cellSize * unitScale; // Convert to meters
    const sectionSize = gridSettings.sectionSize || 10; // Default to every 10th line
    
    // Check if we're using WebGPU (has stricter vertex buffer limits)
    const isWebGPU = scene.getEngine().constructor.name === 'WebGPUEngine';
    
    let gridContainer;
    
    if (gridSettings.infiniteGrid) {
      // Create infinite grid using much larger bounds
      // Reduce size for WebGPU to avoid vertex buffer limits
      const gridSize = isWebGPU ? 200 : 1000; // Smaller grid for WebGPU
      const gridCells = Math.floor(gridSize / cellSize);
      
      // Limit maximum grid cells for WebGPU compatibility
      const maxCells = isWebGPU ? 50 : 500;
      const actualGridCells = Math.min(gridCells, maxCells);
      const regularLines = [];
      const sectionLines = [];
      const halfSize = gridSize / 2;
      
      // Create vertical lines
      for (let i = -actualGridCells; i <= actualGridCells; i++) {
        const x = i * cellSize;
        if (Math.abs(x) <= halfSize) {
          const line = [
            new BABYLON.Vector3(x, 0, -halfSize),
            new BABYLON.Vector3(x, 0, halfSize)
          ];
          
          if (i % sectionSize === 0) {
            sectionLines.push(line);
          } else {
            regularLines.push(line);
          }
        }
      }
      
      // Create horizontal lines
      for (let i = -actualGridCells; i <= actualGridCells; i++) {
        const z = i * cellSize;
        if (Math.abs(z) <= halfSize) {
          const line = [
            new BABYLON.Vector3(-halfSize, 0, z),
            new BABYLON.Vector3(halfSize, 0, z)
          ];
          
          if (i % sectionSize === 0) {
            sectionLines.push(line);
          } else {
            regularLines.push(line);
          }
        }
      }
      
      // Create container for both line systems
      gridContainer = new BABYLON.TransformNode("__grid_container__", scene);
      
      // Create regular grid lines with WebGPU safety limits
      if (regularLines.length > 0) {
        // For WebGPU, limit lines per system to prevent vertex buffer overflow
        const maxLinesPerSystem = isWebGPU ? 100 : regularLines.length;
        const linesToCreate = regularLines.slice(0, maxLinesPerSystem);
        
        const regularGrid = BABYLON.MeshBuilder.CreateLineSystem("__grid_regular__", { lines: linesToCreate }, scene);
        regularGrid.parent = gridContainer;
        regularGrid.isPickable = false;
        regularGrid.material.alpha = 0.3;
        regularGrid.color = BABYLON.Color3.FromHexString(gridSettings.cellColor || '#555555');
      }
      
      // Create section grid lines (major lines) with WebGPU safety limits
      if (sectionLines.length > 0) {
        // For WebGPU, limit lines per system to prevent vertex buffer overflow
        const maxLinesPerSystem = isWebGPU ? 50 : sectionLines.length;
        const linesToCreate = sectionLines.slice(0, maxLinesPerSystem);
        
        const sectionGrid = BABYLON.MeshBuilder.CreateLineSystem("__grid_sections__", { lines: linesToCreate }, scene);
        sectionGrid.parent = gridContainer;
        sectionGrid.isPickable = false;
        sectionGrid.material.alpha = 0.6;
        sectionGrid.color = BABYLON.Color3.FromHexString(gridSettings.sectionColor || '#888888');
      }
    } else {
      // Finite grid
      const gridSize = gridSettings.size * unitScale; // Convert to meters  
      const gridCells = Math.floor(gridSize / cellSize); // Number of cells to fit
      const regularLines = [];
      const sectionLines = [];
      const halfSize = gridSize / 2;
      
      // Create vertical lines
      for (let i = 0; i <= gridCells; i++) {
        const x = (i / gridCells) * gridSize - halfSize;
        const line = [
          new BABYLON.Vector3(x, 0, -halfSize),
          new BABYLON.Vector3(x, 0, halfSize)
        ];
        
        if (i % sectionSize === 0 || i === gridCells) {
          sectionLines.push(line);
        } else {
          regularLines.push(line);
        }
      }
      
      // Create horizontal lines
      for (let i = 0; i <= gridCells; i++) {
        const z = (i / gridCells) * gridSize - halfSize;
        const line = [
          new BABYLON.Vector3(-halfSize, 0, z),
          new BABYLON.Vector3(halfSize, 0, z)
        ];
        
        if (i % sectionSize === 0 || i === gridCells) {
          sectionLines.push(line);
        } else {
          regularLines.push(line);
        }
      }
      
      // Create container for both line systems
      gridContainer = new BABYLON.TransformNode("__grid_container__", scene);
      
      // Create regular grid lines with WebGPU safety limits
      if (regularLines.length > 0) {
        // For WebGPU, limit lines per system to prevent vertex buffer overflow
        const maxLinesPerSystem = isWebGPU ? 100 : regularLines.length;
        const linesToCreate = regularLines.slice(0, maxLinesPerSystem);
        
        const regularGrid = BABYLON.MeshBuilder.CreateLineSystem("__grid_regular__", { lines: linesToCreate }, scene);
        regularGrid.parent = gridContainer;
        regularGrid.isPickable = false;
        regularGrid.material.alpha = 0.3;
        regularGrid.color = BABYLON.Color3.FromHexString(gridSettings.cellColor || '#555555');
      }
      
      // Create section grid lines (major lines) with WebGPU safety limits
      if (sectionLines.length > 0) {
        // For WebGPU, limit lines per system to prevent vertex buffer overflow
        const maxLinesPerSystem = isWebGPU ? 50 : sectionLines.length;
        const linesToCreate = sectionLines.slice(0, maxLinesPerSystem);
        
        const sectionGrid = BABYLON.MeshBuilder.CreateLineSystem("__grid_sections__", { lines: linesToCreate }, scene);
        sectionGrid.parent = gridContainer;
        sectionGrid.isPickable = false;
        sectionGrid.material.alpha = 0.6;
        sectionGrid.color = BABYLON.Color3.FromHexString(gridSettings.sectionColor || '#888888');
      }
    }
    
    // Set container properties
    gridContainer.isPickable = false;
    gridContainer._isSystemObject = true;
    
    // Position the grid
    gridContainer.position = new BABYLON.Vector3(
      gridSettings.position[0],
      gridSettings.position[1], 
      gridSettings.position[2]
    );

    return gridContainer;
  };

  const updateGrid = () => {
    if (!scene) return;

    // Remove existing grid
    if (gridRef.current) {
      gridRef.current.dispose();
      gridRef.current = null;
    }

    // Create new grid
    if (settings.grid.enabled && viewport.showGrid) {
      gridRef.current = createGrid(scene, settings.grid);
    }
  };

  // Update grid when settings change
  useEffect(() => {
    updateGrid();
  }, [
    scene,
    settings.grid.enabled,
    settings.grid.size,
    settings.grid.cellSize,
    settings.grid.unit,
    settings.grid.position,
    settings.grid.cellColor,
    settings.grid.sectionColor,
    settings.grid.infiniteGrid,
    viewport.showGrid
  ]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (gridRef.current) {
        gridRef.current.dispose();
        gridRef.current = null;
      }
    };
  }, []);

  return null; // This is a hook, not a component
}