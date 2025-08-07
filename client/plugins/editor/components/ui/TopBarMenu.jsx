import React, { useState, useEffect } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';
import { useSnapshot } from 'valtio';
import { globalStore, actions } from "@/store.js";
import { autoSaveManager } from '@/plugins/core/AutoSaveManager.js';
import { projectManager } from '@/plugins/projects/projectManager.js';
import ProjectManager from '@/plugins/projects/components/ProjectManager.jsx';

function TopBarMenu() {
  const [activeMenu, setActiveMenu] = useState(null);
  const [isSaving, setIsSaving] = useState(false);
  const [lastSync, setLastSync] = useState(null);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [showSyncTooltip, setShowSyncTooltip] = useState(false);
  const [showUpdateTooltip, setShowUpdateTooltip] = useState(false);
  const [showProjectManager, setShowProjectManager] = useState(false);
  const [selectedTool, setSelectedTool] = useState('select');
  const [flashingTool, setFlashingTool] = useState(null);
  const [menuPosition, setMenuPosition] = useState(null);
  const [isMaximized, setIsMaximized] = useState(false);
  const [isElectron, setIsElectron] = useState(false);
  const [showRendererDropdown, setShowRendererDropdown] = useState(false);
  const [rendererDropdownPosition, setRendererDropdownPosition] = useState(null);
  const { ui, selection, settings } = useSnapshot(globalStore.editor);
  const { transformMode } = selection;
  const { setTransformMode, updateViewportSettings } = actions.editor;
  const currentProject = projectManager.getCurrentProject();

  useEffect(() => {
    const electronCheck = window.electronAPI?.isElectron || false;
    setIsElectron(electronCheck);

    if (electronCheck && window.windowAPI) {
      window.windowAPI.isMaximized().then(setIsMaximized);
      const interval = setInterval(() => {
        window.windowAPI.isMaximized().then(setIsMaximized);
      }, 500);
      return () => clearInterval(interval);
    }
  }, []);

  const handleMinimize = () => {
    if (window.windowAPI) {
      window.windowAPI.minimize();
    }
  };

  const handleMaximize = () => {
    if (window.windowAPI) {
      window.windowAPI.maximize().then(() => {
        setTimeout(() => {
          window.windowAPI.isMaximized().then(setIsMaximized);
        }, 100);
      });
    }
  };

  const handleClose = () => {
    if (window.windowAPI) {
      window.windowAPI.close();
    }
  };

  const handleSave = async () => {
    if (isSaving) return;
    
    try {
      setIsSaving(true);
      await autoSaveManager.saveNow();
      actions.editor.addConsoleMessage('Project saved successfully', 'success');
    } catch (error) {
      console.error('Save failed:', error);
      actions.editor.addConsoleMessage('Failed to save project', 'error');
    } finally {
      setIsSaving(false);
    }
  };

  useEffect(() => {
    const storedProject = projectManager.getCurrentProjectFromStorage()
    if (storedProject?.lastAccessed) {
      setLastSync(new Date(storedProject.lastAccessed))
    }
  }, []);

  useEffect(() => {
    const checkUnsavedChanges = () => {
      setHasUnsavedChanges(autoSaveManager.hasUnsavedChanges())
    }

    checkUnsavedChanges()
    const interval = setInterval(checkUnsavedChanges, 1000)
    return () => clearInterval(interval)
  }, []);

  const formatLastSync = (date) => {
    if (!date) return 'Never synced'
    
    const now = new Date()
    const diff = now - date
    const minutes = Math.floor(diff / 60000)
    const hours = Math.floor(diff / 3600000)
    const days = Math.floor(diff / 86400000)
    
    if (minutes < 1) return 'Just now'
    if (minutes < 60) return `${minutes}m ago`
    if (hours < 24) return `${hours}h ago`
    return `${days}d ago`
  }

  const getSyncStatusInfo = () => {
    if (hasUnsavedChanges) {
      return {
        color: 'bg-yellow-500',
        tooltip: 'Unsaved changes - will auto-save soon'
      }
    }
    return {
      color: 'bg-green-500',
      tooltip: `Last sync: ${formatLastSync(lastSync)}`
    }
  }

  const tools = [
    { id: 'select', icon: Icons.MousePointer || Icons.Select, title: 'Select' },
    { id: 'move', icon: Icons.Move, title: 'Move' },
    { id: 'rotate', icon: Icons.RotateCcw, title: 'Rotate' },
    { id: 'scale', icon: Icons.Maximize, title: 'Scale' },
    { divider: true },
    { id: 'camera', icon: Icons.Video, title: 'Camera' },
    { id: 'paint', icon: Icons.Paintbrush2 || Icons.Paint, title: 'Paint' },
    { divider: true },
    { id: 'undo', icon: Icons.Undo, title: 'Undo' },
    { id: 'redo', icon: Icons.Redo, title: 'Redo' },
  ];

  const getEffectiveSelectedTool = () => {
    if (['select', 'move', 'rotate', 'scale'].includes(transformMode)) {
      return transformMode;
    }
    return selectedTool;
  };

  const handleToolSelect = (toolId) => {
    if (toolId === 'undo' || toolId === 'redo') {
      setFlashingTool(toolId);
      setTimeout(() => setFlashingTool(null), 200);
      console.log(`${toolId} action triggered`);
    } else {
      setSelectedTool(toolId);
      setTransformMode(toolId);
    }
  };

  const rendererOptions = [
    { id: 'webgl', label: 'WebGL' },
    { id: 'webgpu', label: 'WebGPU' }
  ];

  const handleRendererChange = (rendererId) => {
    updateViewportSettings({ renderingEngine: rendererId });
    setShowRendererDropdown(false);
    actions.editor.addConsoleMessage(`Switched to ${rendererId.toUpperCase()} renderer`, 'success');
  };

  const menuStructure = {
    File: [
      { id: 'new', label: 'New Project', icon: Icons.Plus },
      { id: 'open', label: 'Open Project', icon: Icons.Folder },
      { id: 'save', label: 'Save Project', icon: Icons.Save, action: handleSave },
      { id: 'save-as', label: 'Save As...', icon: Icons.Save },
      { divider: true },
      { id: 'import', label: 'Import', icon: Icons.Upload },
      { id: 'export', label: 'Export', icon: Icons.Download },
      { divider: true },
      { id: 'recent', label: 'Recent Projects', icon: Icons.Clock },
    ],
    Edit: [
      { id: 'undo', label: 'Undo', icon: Icons.Undo },
      { id: 'redo', label: 'Redo', icon: Icons.Redo },
      { divider: true },
      { id: 'cut', label: 'Cut', icon: Icons.Scissors },
      { id: 'copy', label: 'Copy', icon: Icons.Copy },
      { id: 'paste', label: 'Paste', icon: Icons.Clipboard },
      { id: 'duplicate', label: 'Duplicate', icon: Icons.Copy },
      { id: 'delete', label: 'Delete', icon: Icons.Trash },
      { divider: true },
      { id: 'select-all', label: 'Select All' },
    ],
    View: [
      { id: 'wireframe', label: 'Wireframe Mode' },
      { id: 'solid', label: 'Solid Mode' },
      { id: 'material', label: 'Material Preview' },
      { id: 'rendered', label: 'Rendered Mode' },
      { divider: true },
      { id: 'grid', label: 'Show Grid' },
      { id: 'axes', label: 'Show Axes' },
      { id: 'statistics', label: 'Show Statistics' },
      { divider: true },
      { id: 'fullscreen', label: 'Fullscreen' },
    ],
    Tools: [
      { id: 'select', label: 'Select Tool', icon: Icons.MousePointer },
      { id: 'move', label: 'Move Tool', icon: Icons.Move },
      { id: 'rotate', label: 'Rotate Tool', icon: Icons.RotateCcw },
      { id: 'scale', label: 'Scale Tool', icon: Icons.Maximize },
      { divider: true },
      { id: 'subdivision', label: 'Subdivision Surface', icon: Icons.Grid3x3 },
      { id: 'mirror', label: 'Mirror Modifier', icon: Icons.Copy },
      { divider: true },
      { id: 'camera', label: 'Camera Tool', icon: Icons.Video },
      { id: 'light', label: 'Light Tool', icon: Icons.Sun },
      { id: 'mesh', label: 'Add Mesh', icon: Icons.Square },
    ],
    Window: [
      { id: 'scene-panel', label: 'Scene Panel' },
      { id: 'properties-panel', label: 'Properties Panel' },
      { id: 'assets-panel', label: 'Assets Panel' },
      { id: 'console-panel', label: 'Console Panel' },
      { divider: true },
      { id: 'settings', label: 'Settings', icon: Icons.Cog },
      { id: 'reset-layout', label: 'Reset Layout' },
    ],
  };

  const handleMenuClick = (menuName, event) => {
    console.log('Menu clicked:', menuName, 'Current active:', activeMenu);
    if (activeMenu === menuName) {
      console.log('Closing menu:', menuName);
      setActiveMenu(null);
      setMenuPosition(null);
    } else {
      console.log('Opening menu:', menuName);
      const rect = event.currentTarget.getBoundingClientRect();
      setMenuPosition({
        left: rect.left,
        top: rect.bottom + 1
      });
      setActiveMenu(menuName);
    }
  };

  const handleItemClick = (item) => {
    setActiveMenu(null);
    setMenuPosition(null);
    if (item.action) {
      item.action();
    } else if (['new', 'open', 'export'].includes(item.id)) {
      setShowProjectManager(true);
    } else if (item.id === 'subdivision') {
      actions.editor.addConsoleMessage('Subdivision Surface applied to selected object', 'success');
    } else if (item.id === 'mirror') {
      actions.editor.addConsoleMessage('Mirror Modifier applied to selected object', 'success');
    } else if (item.id === 'settings') {
      actions.editor.addConsoleMessage('Settings functionality removed', 'info');
    } else {
      console.log('Menu item clicked:', item.id);
      actions.editor.addConsoleMessage(`Menu action: ${item.label}`, 'info');
    }
  };


  return (
    <>
      <div 
        className="relative w-full h-8 bg-gray-900/95 backdrop-blur-sm border-b border-gray-800 flex items-center px-2"
        style={{ 
          WebkitAppRegion: isElectron ? 'drag' : 'auto'
        }}
      >
        <div style={{ WebkitAppRegion: isElectron ? 'no-drag' : 'auto' }}>
          {Object.entries(menuStructure).map(([menuName, items]) => (
            <div key={menuName} className="relative inline-block">
              <button
                onClick={(e) => handleMenuClick(menuName, e)}
                onMouseEnter={(e) => {
                  console.log('Hovering over menu:', menuName, 'Current active menu:', activeMenu);
                  if (activeMenu) {
                    console.log('Switching from', activeMenu, 'to', menuName);
                    const rect = e.currentTarget.getBoundingClientRect();
                    setMenuPosition({
                      left: rect.left,
                      top: rect.bottom + 1
                    });
                    setActiveMenu(menuName);
                  } else {
                    console.log('No active menu, not switching');
                  }
                }}
                className={`px-3 py-1 text-sm text-gray-300 hover:bg-gray-700/50 rounded transition-colors ${
                  activeMenu === menuName ? 'bg-gray-700/50' : ''
                }`}
              >
                {menuName}
              </button>
            </div>
          ))}
        </div>
        
        <div className="flex-1" />
        <div className="flex items-center gap-3 text-xs text-gray-500">
          <div className="flex items-center gap-2">
            <span className="text-gray-400">
              {currentProject?.name || 'Renzora Engine v1.0.0'}
            </span>
            {currentProject?.name && (
              <div 
                className={`w-1.5 h-1.5 ${getSyncStatusInfo().color} rounded-full cursor-pointer relative`}
                onMouseEnter={() => setShowSyncTooltip(true)}
                onMouseLeave={() => setShowSyncTooltip(false)}
              >
                {showSyncTooltip && (
                  <div className="absolute bottom-full left-1/2 transform -translate-x-1/2 mb-2 px-2 py-1 bg-gray-900/95 text-white text-xs rounded whitespace-nowrap z-[120]">
                    {getSyncStatusInfo().tooltip}
                    <div className="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-gray-900/95" />
                  </div>
                )}
              </div>
            )}
          </div>
          
          {currentProject?.name && (
            <div className="flex items-center gap-2">
              <span className="text-gray-600">•</span>
              <span 
                className="text-orange-400 text-xs font-medium flex items-center gap-1 cursor-pointer relative"
                onMouseEnter={() => setShowUpdateTooltip(true)}
                onMouseLeave={() => setShowUpdateTooltip(false)}
              >
                Renzora Engine v1.0.0
                <Icons.Download className="w-3 h-3" />
                {showUpdateTooltip && (
                  <div className="absolute right-full top-1/2 transform -translate-y-1/2 mr-2 px-2 py-1 bg-gray-900/95 text-white text-xs rounded whitespace-nowrap z-[120]">
                    Update to v1.1.0
                    <div className="absolute left-full top-1/2 transform -translate-y-1/2 border-4 border-transparent border-l-gray-900/95" />
                  </div>
                )}
              </span>
              <span className="text-gray-600">•</span>
              <div className="relative">
                <button
                  onClick={(e) => {
                    if (showRendererDropdown) {
                      setShowRendererDropdown(false);
                      setRendererDropdownPosition(null);
                    } else {
                      const rect = e.currentTarget.getBoundingClientRect();
                      const dropdownWidth = 128;
                      setRendererDropdownPosition({
                        left: rect.right - dropdownWidth,
                        top: rect.bottom + 4
                      });
                      setShowRendererDropdown(true);
                    }
                  }}
                  className="text-blue-400 font-medium hover:text-blue-300 transition-colors px-2 py-1 rounded hover:bg-gray-700/50 flex items-center gap-1"
                  style={{ WebkitAppRegion: 'no-drag' }}
                >
                  {(settings.viewport.renderingEngine || 'webgl').toUpperCase()}
                  <svg 
                    className={`w-3 h-3 transition-transform ${showRendererDropdown ? 'rotate-180' : ''}`} 
                    fill="currentColor" 
                    viewBox="0 0 20 20"
                  >
                    <path fillRule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clipRule="evenodd" />
                  </svg>
                </button>
                
              </div>
            </div>
          )}
        </div>

        {isElectron && (
          <div className="flex items-center ml-2" style={{ WebkitAppRegion: 'no-drag' }}>
            <button
              onClick={handleMinimize}
              className="w-8 h-8 flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-700/50 transition-colors"
              title="Minimize"
            >
              <svg width="10" height="1" viewBox="0 0 10 1" fill="currentColor">
                <rect width="10" height="1" />
              </svg>
            </button>
            <button
              onClick={handleMaximize}
              className="w-8 h-8 flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-700/50 transition-colors"
              title={isMaximized ? "Restore" : "Maximize"}
            >
              {isMaximized ? (
                <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
                  <path d="M2.5,2.5 L2.5,0.5 L10,0.5 L10,8 L8,8 L8,2.5 L2.5,2.5 Z M0,2 L0,10 L8,10 L8,8 L2,8 L2,2 L0,2 Z" />
                </svg>
              ) : (
                <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
                  <rect width="10" height="10" fill="none" stroke="currentColor" strokeWidth="1" />
                </svg>
              )}
            </button>
            <button
              onClick={handleClose}
              className="w-8 h-8 flex items-center justify-center text-gray-400 hover:text-white hover:bg-red-600/80 transition-colors"
              title="Close"
            >
              <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
                <path d="M0.7,0 L5,4.3 L9.3,0 L10,0.7 L5.7,5 L10,9.3 L9.3,10 L5,5.7 L0.7,10 L0,9.3 L4.3,5 L0,0.7 L0.7,0 Z" />
              </svg>
            </button>
          </div>
        )}
      </div>
      
      {activeMenu && menuPosition && (
        <>
          <div 
            className="fixed z-[100]"
            style={{
              top: '32px',
              left: 0,
              right: 0,
              bottom: 0,
              pointerEvents: 'auto'
            }}
            onClick={() => {
              setActiveMenu(null);
              setMenuPosition(null);
            }}
          />
          
          <div 
            className="fixed w-56 bg-gradient-to-br from-gray-900/98 to-gray-950/98 backdrop-blur-sm rounded-lg shadow-[0_20px_25px_-5px_rgba(0,0,0,0.4)] overflow-hidden z-[110] border border-gray-700/50"
            style={{
              left: menuPosition.left,
              top: menuPosition.top
            }}
          >
            <div className="p-1">
              {menuStructure[activeMenu]?.map((item, index) => (
                item.divider ? (
                  <div key={index} className="border-t border-gray-700/50 my-1 mx-2" />
                ) : (
                  <button
                    key={item.id}
                    className="w-full px-3 py-1.5 text-left text-sm text-gray-300 hover:bg-gradient-to-r hover:from-blue-600/90 hover:to-blue-500/90 hover:text-white flex items-center justify-between transition-all duration-150 group relative rounded-md hover:shadow-lg"
                    onClick={() => handleItemClick(item)}
                  >
                    <div className="flex items-center gap-2">
                      {item.icon && (
                        <span className="w-4 h-4 flex items-center justify-center text-gray-400 group-hover:text-white">
                          {item.id === 'save' && isSaving ? (
                            <div className="w-3 h-3 border-2 border-gray-400 border-t-transparent rounded-full animate-spin" />
                          ) : (
                            <item.icon className="w-3.5 h-3.5" />
                          )}
                        </span>
                      )}
                      <span className="font-normal">
                        {item.id === 'save' && isSaving ? 'Saving...' : item.label}
                      </span>
                    </div>
                  </button>
                )
              ))}
            </div>
          </div>
        </>
      )}
      
      {showRendererDropdown && rendererDropdownPosition && (
        <>
          <div 
            className="fixed inset-0 z-[200]" 
            onClick={() => {
              setShowRendererDropdown(false);
              setRendererDropdownPosition(null);
            }}
          />
          <div 
            className="fixed w-32 bg-gray-800/95 backdrop-blur-sm rounded-lg shadow-xl border border-gray-600/50 z-[210]"
            style={{
              left: rendererDropdownPosition.left,
              top: rendererDropdownPosition.top
            }}
          >
            {rendererOptions.map((option) => (
              <button
                key={option.id}
                onClick={() => handleRendererChange(option.id)}
                className={`w-full px-3 py-2 text-left text-sm transition-colors flex items-center gap-2 first:rounded-t-lg last:rounded-b-lg ${
                  settings.viewport.renderingEngine === option.id
                    ? 'bg-green-600/90 text-white'
                    : 'text-gray-300 hover:bg-gray-900/60 hover:text-white'
                }`}
              >
                {option.label}
                {settings.viewport.renderingEngine === option.id && (
                  <svg className="w-3 h-3 ml-auto" fill="currentColor" viewBox="0 0 20 20" strokeWidth="2">
                    <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" />
                  </svg>
                )}
              </button>
            ))}
          </div>
        </>
      )}

      {showProjectManager && (
        <ProjectManager
          onProjectLoad={(name, path) => {
            console.log(`Project loaded: ${name} at ${path}`)
            actions.editor.addConsoleMessage(`Project "${name}" loaded successfully`, 'success')
          }}
          onClose={() => setShowProjectManager(false)}
        />
      )}

      
    </>
  );
}

export default TopBarMenu;