import React, { useState, useEffect, useRef } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';
import { useSnapshot } from 'valtio';
import { globalStore, actions } from "@/store.js";

export default function CameraHelpers() {
  const [isExpanded, setIsExpanded] = useState(false);
  const cameraRef = useRef(null);
  const { camera, viewport, settings } = useSnapshot(globalStore.editor);
  const { setCameraSpeed, setCameraSensitivity, setRenderMode } = actions.editor;
  const cameraSpeed = camera.speed || 5;
  const mouseSensitivity = camera.mouseSensitivity || 0.002;
  const renderMode = viewport.renderMode || 'solid';
  
  const renderModes = [
    { id: 'wireframe', label: 'Wireframe', icon: Icons.Grid3x3 },
    { id: 'solid', label: 'Solid', icon: Icons.Cube },
    { id: 'material', label: 'Material', icon: Icons.Palette },
    { id: 'rendered', label: 'Rendered', icon: Icons.Sun }
  ];
  
  const speedPresets = [
    { value: 1, label: 'Slow' },
    { value: 5, label: 'Normal' },
    { value: 10, label: 'Fast' },
    { value: 20, label: 'Very Fast' }
  ];

  useEffect(() => {
    const handleClickOutside = (event) => {
      if (cameraRef.current && !cameraRef.current.contains(event.target)) {
        setIsExpanded(false);
      }
    };

    if (isExpanded) {
      document.addEventListener('mousedown', handleClickOutside);
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isExpanded]);

  return (
    <div className="relative group" ref={cameraRef}>
      <button
        className={`pl-2 pr-1 py-1 text-xs rounded transition-colors cursor-pointer ${
          isExpanded
            ? 'bg-blue-600 text-white'
            : 'text-gray-400 hover:text-gray-200 hover:bg-slate-800'
        }`}
        onClick={() => setIsExpanded(!isExpanded)}
        title="Camera Settings"
      >
        <div className="flex items-center gap-0.5">
          <Icons.Video className="w-4 h-4" />
          <svg className="w-2 h-2" fill="currentColor" viewBox="0 0 20 20">
            <path fillRule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clipRule="evenodd" />
          </svg>
        </div>
        
        <div className="absolute right-full mr-2 top-1/2 transform -translate-y-1/2 bg-gray-900/95 text-white text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
          Camera Settings
          <div className="absolute left-full top-1/2 transform -translate-y-1/2 border-4 border-transparent border-l-gray-900/95" />
        </div>
      </button>
      
      {isExpanded && (
        <div className="absolute top-full right-0 mt-2 w-64 bg-gray-900/95 backdrop-blur-sm border border-gray-700 rounded-lg shadow-xl space-y-4 text-white text-xs pointer-events-auto z-50 p-4">
          <div>
            <label className="block font-medium text-gray-300 mb-2">
              Camera Speed: {cameraSpeed}
            </label>
            <div className="grid grid-cols-2 gap-1 mb-2">
              {speedPresets.map((preset) => (
                <button
                  key={preset.value}
                  onClick={() => setCameraSpeed(preset.value)}
                  className={`px-2 py-1 text-xs rounded transition-colors ${
                    cameraSpeed === preset.value
                      ? 'bg-blue-600 text-white'
                      : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
                  }`}
                >
                  {preset.label}
                </button>
              ))}
            </div>
            <input
              type="range"
              min="0.5"
              max="50"
              step="0.5"
              value={cameraSpeed}
              onChange={(e) => setCameraSpeed(parseFloat(e.target.value))}
              className="w-full h-2 bg-gray-700 rounded-lg appearance-none cursor-pointer slider"
            />
          </div>
          
          <div>
            <label className="block font-medium text-gray-300 mb-2">
              Mouse Sensitivity: {(mouseSensitivity * 1000).toFixed(1)}
            </label>
            <input
              type="range"
              min="0.001"
              max="0.01"
              step="0.0001"
              value={mouseSensitivity}
              onChange={(e) => setCameraSensitivity(parseFloat(e.target.value))}
              className="w-full h-2 bg-gray-700 rounded-lg appearance-none cursor-pointer slider"
            />
          </div>
          
          <div>
            <label className="block font-medium text-gray-300 mb-2">
              Render Mode
            </label>
            <div className="grid grid-cols-2 gap-1">
              {renderModes.map((mode) => (
                <button
                  key={mode.id}
                  onClick={() => setRenderMode(mode.id)}
                  className={`flex items-center gap-2 px-2 py-2 text-xs rounded transition-colors ${
                    renderMode === mode.id
                      ? 'bg-blue-600 text-white'
                      : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
                  }`}
                  title={mode.label}
                >
                  <mode.icon className="w-3 h-3" />
                  <span>{mode.label}</span>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}