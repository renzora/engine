import React, { useState, useEffect, useRef } from 'react';
import { useSnapshot } from 'valtio';
import { globalStore, actions } from "@/store.js";

export default function GridHelpers() {
  const [isExpanded, setIsExpanded] = useState(false);
  const gridRef = useRef(null);
  const { viewport, settings } = useSnapshot(globalStore.editor);
  const { setGridSnapping, updateGridSettings } = actions.editor;
  
  const gridSnapping = viewport.gridSnapping || false;
  const gridSettings = settings.grid;

  useEffect(() => {
    const handleClickOutside = (event) => {
      if (gridRef.current && !gridRef.current.contains(event.target)) {
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
    <div ref={gridRef} className="relative group">
      <button
        className={`pl-2 pr-1 py-1 text-xs rounded transition-colors cursor-pointer ${
          isExpanded
            ? 'bg-blue-600 text-white'
            : 'text-gray-400 hover:text-gray-200 hover:bg-slate-800'
        }`}
        onClick={() => setIsExpanded(!isExpanded)}
        title="Grid Settings"
      >
        <div className="flex items-center gap-0.5">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="w-4 h-4">
            <rect x="3" y="3" width="18" height="18" rx="2"/>
            <path d="M9 3v18"/>
            <path d="M15 3v18"/>
            <path d="M3 9h18"/>
            <path d="M3 15h18"/>
          </svg>
          <svg className="w-2 h-2" fill="currentColor" viewBox="0 0 20 20">
            <path fillRule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clipRule="evenodd" />
          </svg>
        </div>
        
        <div className="absolute right-full mr-2 top-1/2 transform -translate-y-1/2 bg-gray-900/95 text-white text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
          Grid Settings
          <div className="absolute left-full top-1/2 transform -translate-y-1/2 border-4 border-transparent border-l-gray-900/95" />
        </div>
      </button>
      
      {isExpanded && (
        <div className="absolute top-full right-0 mt-2 w-72 bg-gray-900/95 backdrop-blur-sm border border-gray-700 rounded-lg shadow-xl space-y-4 text-white text-xs pointer-events-auto z-50 p-4">
          <div>
            <label className="block font-medium text-gray-300 mb-2">
              Grid Settings
            </label>
            <div className="space-y-3">
              <div className="flex items-center justify-between p-2 bg-gray-800/50 rounded border border-gray-700">
                <label className="text-xs font-medium text-gray-300">Enable Grid</label>
                <button
                  onClick={() => updateGridSettings({ enabled: !gridSettings.enabled })}
                  className={`relative inline-flex h-5 w-9 items-center rounded-full transition-all duration-200 ${
                    gridSettings.enabled ? 'bg-blue-500 shadow-lg shadow-blue-500/30' : 'bg-gray-600'
                  }`}
                >
                  <span
                    className={`inline-block h-3 w-3 transform rounded-full bg-white transition-transform duration-200 shadow-sm ${
                      gridSettings.enabled ? 'translate-x-5' : 'translate-x-1'
                    }`}
                  />
                </button>
              </div>

              {gridSettings.enabled && (
                <div className="space-y-3 pt-2 border-t border-gray-700">
                  <div>
                    <label className="block text-xs text-gray-400 mb-1">Units</label>
                    <select
                      value={gridSettings.unit || 'meters'}
                      onChange={(e) => updateGridSettings({ unit: e.target.value })}
                      className="w-full bg-gray-800 border border-gray-600 text-white text-xs p-1.5 rounded focus:outline-none focus:ring-1 focus:ring-blue-500"
                    >
                      <option value="meters">Meters (m)</option>
                      <option value="centimeters">Centimeters (cm)</option>
                      <option value="millimeters">Millimeters (mm)</option>
                      <option value="feet">Feet (ft)</option>
                      <option value="inches">Inches (in)</option>
                    </select>
                  </div>

                  <div className="grid grid-cols-2 gap-2">
                    {!gridSettings.infiniteGrid && (
                      <div>
                        <label className="block text-xs text-gray-400 mb-1">Size ({gridSettings.unit || 'meters'})</label>
                        <input
                          type="number"
                          step={gridSettings.unit === 'millimeters' ? "100" : gridSettings.unit === 'centimeters' ? "10" : gridSettings.unit === 'inches' ? "12" : "1"}
                          min={gridSettings.unit === 'millimeters' ? "1000" : gridSettings.unit === 'centimeters' ? "100" : "1"}
                          max={gridSettings.unit === 'millimeters' ? "50000" : gridSettings.unit === 'centimeters' ? "5000" : gridSettings.unit === 'inches' ? "600" : "100"}
                          value={gridSettings.size}
                          onChange={(e) => updateGridSettings({ size: parseInt(e.target.value) || (gridSettings.unit === 'millimeters' ? 20000 : gridSettings.unit === 'centimeters' ? 2000 : gridSettings.unit === 'inches' ? 240 : 20) })}
                          className="w-full bg-gray-800 border border-gray-600 text-white text-xs p-1.5 rounded focus:outline-none focus:ring-1 focus:ring-blue-500"
                        />
                      </div>
                    )}
                    <div>
                      <label className="block text-xs text-gray-400 mb-1">Cell Size ({gridSettings.unit || 'meters'})</label>
                      <input
                        type="number"
                        step={gridSettings.unit === 'millimeters' ? "10" : gridSettings.unit === 'centimeters' ? "1" : gridSettings.unit === 'inches' ? "1" : "0.1"}
                        min={gridSettings.unit === 'millimeters' ? "10" : gridSettings.unit === 'centimeters' ? "1" : gridSettings.unit === 'inches' ? "1" : "0.1"}
                        max={gridSettings.unit === 'millimeters' ? "5000" : gridSettings.unit === 'centimeters' ? "500" : gridSettings.unit === 'inches' ? "60" : "10"}
                        value={gridSettings.cellSize}
                        onChange={(e) => updateGridSettings({ cellSize: parseFloat(e.target.value) || (gridSettings.unit === 'millimeters' ? 1000 : gridSettings.unit === 'centimeters' ? 100 : gridSettings.unit === 'inches' ? 12 : 1) })}
                        className="w-full bg-gray-800 border border-gray-600 text-white text-xs p-1.5 rounded focus:outline-none focus:ring-1 focus:ring-blue-500"
                      />
                    </div>
                  </div>

                  <div>
                    <label className="block text-xs text-gray-400 mb-1">Position</label>
                    <div className="grid grid-cols-3 gap-1">
                      <div className="relative">
                        <span className="absolute left-0 top-0 bottom-0 w-6 flex items-center justify-center text-[10px] text-gray-300 pointer-events-none font-medium bg-gray-700 border-t border-l border-b border-r border-gray-600 rounded-l">X</span>
                        <input
                          type="number"
                          step="0.1"
                          value={gridSettings.position[0]}
                          onChange={(e) => {
                            const newPos = [...gridSettings.position];
                            newPos[0] = parseFloat(e.target.value) || 0;
                            updateGridSettings({ position: newPos });
                          }}
                          className="w-full bg-gray-800 border border-gray-600 text-white text-xs p-1.5 pl-7 pr-1.5 rounded focus:outline-none focus:ring-1 focus:ring-blue-500 text-center"
                        />
                      </div>
                      <div className="relative">
                        <span className="absolute left-0 top-0 bottom-0 w-6 flex items-center justify-center text-[10px] text-gray-300 pointer-events-none font-medium bg-gray-700 border-t border-l border-b border-r border-gray-600 rounded-l">Y</span>
                        <input
                          type="number"
                          step="0.1"
                          value={gridSettings.position[1]}
                          onChange={(e) => {
                            const newPos = [...gridSettings.position];
                            newPos[1] = parseFloat(e.target.value) || 0;
                            updateGridSettings({ position: newPos });
                          }}
                          className="w-full bg-gray-800 border border-gray-600 text-white text-xs p-1.5 pl-7 pr-1.5 rounded focus:outline-none focus:ring-1 focus:ring-blue-500 text-center"
                        />
                      </div>
                      <div className="relative">
                        <span className="absolute left-0 top-0 bottom-0 w-6 flex items-center justify-center text-[10px] text-gray-300 pointer-events-none font-medium bg-gray-700 border-t border-l border-b border-r border-gray-600 rounded-l">Z</span>
                        <input
                          type="number"
                          step="0.1"
                          value={gridSettings.position[2]}
                          onChange={(e) => {
                            const newPos = [...gridSettings.position];
                            newPos[2] = parseFloat(e.target.value) || 0;
                            updateGridSettings({ position: newPos });
                          }}
                          className="w-full bg-gray-800 border border-gray-600 text-white text-xs p-1.5 pl-7 pr-1.5 rounded focus:outline-none focus:ring-1 focus:ring-blue-500 text-center"
                        />
                      </div>
                    </div>
                  </div>

                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="block text-xs text-gray-400 mb-1">Cell Color</label>
                      <div className="flex items-center gap-1">
                        <input
                          type="color"
                          value={gridSettings.cellColor}
                          onChange={(e) => updateGridSettings({ cellColor: e.target.value })}
                          className="w-6 h-6 rounded border border-gray-600 bg-gray-800 cursor-pointer"
                        />
                        <div className="flex-1 bg-gray-800 border border-gray-600 rounded px-1.5 py-1">
                          <div className="text-xs text-gray-300">{gridSettings.cellColor.toUpperCase()}</div>
                        </div>
                      </div>
                    </div>
                    <div>
                      <label className="block text-xs text-gray-400 mb-1">Section Color</label>
                      <div className="flex items-center gap-1">
                        <input
                          type="color"
                          value={gridSettings.sectionColor}
                          onChange={(e) => updateGridSettings({ sectionColor: e.target.value })}
                          className="w-6 h-6 rounded border border-gray-600 bg-gray-800 cursor-pointer"
                        />
                        <div className="flex-1 bg-gray-800 border border-gray-600 rounded px-1.5 py-1">
                          <div className="text-xs text-gray-300">{gridSettings.sectionColor.toUpperCase()}</div>
                        </div>
                      </div>
                    </div>
                  </div>

                  <div className="grid grid-cols-2 gap-2">
                    <div className="flex items-center justify-between p-2 bg-gray-800/50 rounded border border-gray-700">
                      <label className="text-xs font-medium text-gray-300">Infinite</label>
                      <button
                        onClick={() => updateGridSettings({ infiniteGrid: !gridSettings.infiniteGrid })}
                        className={`relative inline-flex h-4 w-8 items-center rounded-full transition-all duration-200 ${
                          gridSettings.infiniteGrid ? 'bg-blue-500 shadow-lg shadow-blue-500/30' : 'bg-gray-600'
                        }`}
                      >
                        <span
                          className={`inline-block h-2.5 w-2.5 transform rounded-full bg-white transition-transform duration-200 shadow-sm ${
                            gridSettings.infiniteGrid ? 'translate-x-4' : 'translate-x-0.5'
                          }`}
                        />
                      </button>
                    </div>

                    <div className="flex items-center justify-between p-2 bg-gray-800/50 rounded border border-gray-700">
                      <label className="text-xs font-medium text-gray-300">Snapping</label>
                      <button
                        onClick={() => setGridSnapping(!gridSnapping)}
                        className={`relative inline-flex h-4 w-8 items-center rounded-full transition-all duration-200 ${
                          gridSnapping ? 'bg-yellow-500 shadow-lg shadow-yellow-500/30' : 'bg-gray-600'
                        }`}
                      >
                        <span
                          className={`inline-block h-2.5 w-2.5 transform rounded-full bg-white transition-transform duration-200 shadow-sm ${
                            gridSnapping ? 'translate-x-4' : 'translate-x-0.5'
                          }`}
                        />
                      </button>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}