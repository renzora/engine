import { useEffect, useRef } from 'react';
import { useSnapshot } from 'valtio';
import { globalStore } from '@/store.js';
import * as BABYLON from '@babylonjs/core';

export function useGrid(scene) {
  const settings = useSnapshot(globalStore.editor.settings);
  const viewport = useSnapshot(globalStore.editor.viewport);
  const gridRef = useRef(null);
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
      
      gridContainer = new BABYLON.TransformNode("__grid_container__", scene);
      
      if (regularLines.length > 0) {
        const maxLinesPerSystem = isWebGPU ? 100 : regularLines.length;
        const linesToCreate = regularLines.slice(0, maxLinesPerSystem);
        
        const regularGrid = BABYLON.MeshBuilder.CreateLineSystem("__grid_regular__", { lines: linesToCreate }, scene);
        regularGrid.parent = gridContainer;
        regularGrid.isPickable = false;
        regularGrid.material.alpha = 0.3;
        regularGrid.color = BABYLON.Color3.FromHexString(gridSettings.cellColor || '#555555');
      }
      
      if (sectionLines.length > 0) {
        const maxLinesPerSystem = isWebGPU ? 50 : sectionLines.length;
        const linesToCreate = sectionLines.slice(0, maxLinesPerSystem);
        const sectionGrid = BABYLON.MeshBuilder.CreateLineSystem("__grid_sections__", { lines: linesToCreate }, scene);
        sectionGrid.parent = gridContainer;
        sectionGrid.isPickable = false;
        sectionGrid.material.alpha = 0.6;
        sectionGrid.color = BABYLON.Color3.FromHexString(gridSettings.sectionColor || '#888888');
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
          new BABYLON.Vector3(x, 0, -halfSize),
          new BABYLON.Vector3(x, 0, halfSize)
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
          new BABYLON.Vector3(-halfSize, 0, z),
          new BABYLON.Vector3(halfSize, 0, z)
        ];
        
        if (i % sectionSize === 0 || i === gridCells) {
          sectionLines.push(line);
        } else {
          regularLines.push(line);
        }
      }
      
      gridContainer = new BABYLON.TransformNode("__grid_container__", scene);
      
      if (regularLines.length > 0) {
        const maxLinesPerSystem = isWebGPU ? 100 : regularLines.length;
        const linesToCreate = regularLines.slice(0, maxLinesPerSystem);
        
        const regularGrid = BABYLON.MeshBuilder.CreateLineSystem("__grid_regular__", { lines: linesToCreate }, scene);
        regularGrid.parent = gridContainer;
        regularGrid.isPickable = false;
        regularGrid.material.alpha = 0.3;
        regularGrid.color = BABYLON.Color3.FromHexString(gridSettings.cellColor || '#555555');
      }
      
      if (sectionLines.length > 0) {
        const maxLinesPerSystem = isWebGPU ? 50 : sectionLines.length;
        const linesToCreate = sectionLines.slice(0, maxLinesPerSystem);
        const sectionGrid = BABYLON.MeshBuilder.CreateLineSystem("__grid_sections__", { lines: linesToCreate }, scene);
        sectionGrid.parent = gridContainer;
        sectionGrid.isPickable = false;
        sectionGrid.material.alpha = 0.6;
        sectionGrid.color = BABYLON.Color3.FromHexString(gridSettings.sectionColor || '#888888');
      }
    }
    
    gridContainer.isPickable = false;
    gridContainer._isSystemObject = true;
    gridContainer.position = new BABYLON.Vector3(
      gridSettings.position[0],
      gridSettings.position[1], 
      gridSettings.position[2]
    );

    return gridContainer;
  };

  const updateGrid = () => {
    if (!scene) return;

    if (gridRef.current) {
      gridRef.current.dispose();
      gridRef.current = null;
    }

    if (settings.grid.enabled && viewport.showGrid) {
      gridRef.current = createGrid(scene, settings.grid);
    }
  };

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

  useEffect(() => {
    return () => {
      if (gridRef.current) {
        gridRef.current.dispose();
        gridRef.current = null;
      }
    };
  }, []);

  return null;
}