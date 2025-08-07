import React, { useState, useRef, useEffect } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';
import { useSnapshot } from 'valtio';
import { globalStore, actions } from "@/store.js";

const defaultTools = [
  { id: 'scene', icon: Icons.Scene, title: 'Scene' },
];

const defaultBottomTools = [
  { id: 'add', icon: Icons.PlusCircle, title: 'Add' },
  { id: 'settings', icon: Icons.Settings, title: 'Settings' },
  { id: 'fullscreen', icon: Icons.Fullscreen, title: 'Fullscreen' },
];

// Workflow-based tool configurations for right panel
const workflowTools = {
  '3d-viewport': [
    'scene'
  ],
  'daw-editor': [
    'scene'
  ],
  'material-editor': [
    'scene'
  ],
  'node-editor': [
    'scene'
  ],
  'animation-editor': [
    'scene'
  ],
  'text-editor': [
    'scene'
  ],
  'video-editor': [
    'scene'
  ],
  'photo-editor': [
    'scene'
  ],
  'model-preview': [
    'scene'
  ],
  'default': [
    'scene'
  ]
};

function Toolbar({ selectedTool, onToolSelect, scenePanelOpen, onScenePanelToggle }) {
  const { ui, viewport, settings } = useSnapshot(globalStore.editor);
  const { toolbarTabOrder, toolbarBottomTabOrder } = ui;
  const panelPosition = settings.editor.panelPosition || 'right';
  const isPanelOnLeft = panelPosition === 'left';
  
  // Show tooltips to the right when panel is on left (into panel content)
  // Show tooltips to the left when panel is on right (into viewport)
  const shouldTooltipGoRight = isPanelOnLeft;
  const { setToolbarTabOrder, setToolbarBottomTabOrder } = actions.editor;
  
  // Get current active viewport type for workflow filtering
  const getCurrentWorkflow = () => {
    if (!viewport.tabs || viewport.tabs.length === 0) {
      return 'default';
    }
    const activeTabData = viewport.tabs.find(tab => tab.id === viewport.activeTabId);
    return activeTabData?.type || 'default';
  };
  
  // Create ordered tools based on workflow and stored order
  const getOrderedTools = () => {
    const currentWorkflow = getCurrentWorkflow();
    const allowedToolIds = workflowTools[currentWorkflow] || workflowTools['default'];
    
    const toolsMap = defaultTools.reduce((map, tool) => {
      map[tool.id] = tool;
      return map;
    }, {});
    
    // Migration: Add missing tools to stored order if they don't exist
    let currentTabOrder = toolbarTabOrder || [];
    const missingTools = defaultTools
      .filter(tool => !currentTabOrder.includes(tool.id))
      .map(tool => tool.id);
    
    if (missingTools.length > 0) {
      currentTabOrder = [...currentTabOrder, ...missingTools];
      // Update store with new tools
      setToolbarTabOrder(currentTabOrder);
    }
    
    // Filter tools based on current workflow, then apply user ordering
    if (!currentTabOrder || !Array.isArray(currentTabOrder)) {
      return defaultTools.filter(tool => allowedToolIds.includes(tool.id));
    }
    
    const workflowFilteredTools = currentTabOrder
      .filter(id => allowedToolIds.includes(id))
      .map(id => toolsMap[id])
      .filter(Boolean);
    
    return workflowFilteredTools;
  };
  
  const getOrderedBottomTools = () => {
    if (!toolbarBottomTabOrder || !Array.isArray(toolbarBottomTabOrder)) {
      return defaultBottomTools;
    }
    const toolsMap = defaultBottomTools.reduce((map, tool) => {
      map[tool.id] = tool;
      return map;
    }, {});
    
    // Get tools from stored order
    const orderedTools = toolbarBottomTabOrder.map(id => toolsMap[id]).filter(Boolean);
    
    // Add any new default tools that aren't in the stored order
    const existingIds = new Set(toolbarBottomTabOrder);
    const newTools = defaultBottomTools.filter(tool => !existingIds.has(tool.id));
    
    return [...orderedTools, ...newTools];
  };
  
  const [tools, setTools] = useState(() => getOrderedTools());
  const [bottomTools, setBottomTools] = useState(() => getOrderedBottomTools());
  
  // Update tools when store order or viewport changes
  useEffect(() => {
    const orderedTools = getOrderedTools();
    setTools(orderedTools);
  }, [toolbarTabOrder, viewport.activeTabId]);
  
  useEffect(() => {
    const toolsMap = defaultBottomTools.reduce((map, tool) => {
      map[tool.id] = tool;
      return map;
    }, {});
    
    // Get tools from stored order
    const orderedTools = toolbarBottomTabOrder.map(id => toolsMap[id]).filter(Boolean);
    
    // Add any new default tools that aren't in the stored order
    const existingIds = new Set(toolbarBottomTabOrder);
    const newTools = defaultBottomTools.filter(tool => !existingIds.has(tool.id));
    
    setBottomTools([...orderedTools, ...newTools]);
  }, [toolbarBottomTabOrder]);
  
  const [dragState, setDragState] = useState({
    isDragging: false,
    draggedTool: null,
    dragOverTool: null,
    draggedFromBottom: false
  });

  // Drag and drop handlers
  const handleDragStart = (e, tool, isFromBottom = false) => {
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/html', '');
    
    // Create custom drag image with background and border
    const dragElement = e.currentTarget.cloneNode(true);
    dragElement.style.position = 'absolute';
    dragElement.style.top = '-1000px';
    dragElement.style.left = '-1000px';
    dragElement.style.background = 'linear-gradient(to bottom, rgb(51 65 85 / 0.95), rgb(15 23 42 / 0.98))';
    dragElement.style.border = '1px solid rgb(148 163 184 / 0.5)';
    dragElement.style.borderRadius = '8px';
    dragElement.style.padding = '8px';
    dragElement.style.boxShadow = '0 25px 50px -12px rgb(0 0 0 / 0.5)';
    dragElement.style.transform = 'scale(1.1)';
    dragElement.style.pointerEvents = 'none';
    dragElement.style.zIndex = '9999';
    
    // Ensure icon colors are preserved
    const icon = dragElement.querySelector('svg');
    if (icon) {
      icon.style.color = '#e2e8f0'; // light gray
    }
    
    document.body.appendChild(dragElement);
    e.dataTransfer.setDragImage(dragElement, 24, 24);
    
    // Clean up drag image after a short delay
    setTimeout(() => {
      if (document.body.contains(dragElement)) {
        document.body.removeChild(dragElement);
      }
    }, 100);
    
    setDragState({
      isDragging: true,
      draggedTool: tool,
      dragOverTool: null,
      draggedFromBottom: isFromBottom
    });
  };

  const handleDragOver = (e, tool, isBottomArea = false) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    
    if (dragState.draggedTool && dragState.draggedTool.id !== tool.id) {
      setDragState(prev => ({ ...prev, dragOverTool: tool }));
    }
  };

  const handleDragLeave = (e) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const { clientX, clientY } = e;
    
    if (clientX < rect.left || clientX > rect.right || clientY < rect.top || clientY > rect.bottom) {
      setDragState(prev => ({ ...prev, dragOverTool: null }));
    }
  };

  const handleDrop = (e, dropTool, isBottomArea = false) => {
    e.preventDefault();
    e.stopPropagation();
    
    if (!dragState.draggedTool || dragState.draggedTool.id === dropTool.id) {
      setDragState({
        isDragging: false,
        draggedTool: null,
        dragOverTool: null,
        draggedFromBottom: false
      });
      return;
    }

    const sourceArray = dragState.draggedFromBottom ? bottomTools : tools;
    const targetArray = isBottomArea ? bottomTools : tools;
    const setSourceArray = dragState.draggedFromBottom ? setBottomTools : setTools;
    const setTargetArray = isBottomArea ? setBottomTools : setTools;

    // Handle cross-section movement (top tools <-> bottom tools)
    if (dragState.draggedFromBottom !== isBottomArea) {
      // Remove from source array
      const newSourceArray = sourceArray.filter(tool => tool.id !== dragState.draggedTool.id);
      setSourceArray(newSourceArray);
      
      // Add to target array at the dropped position
      const dropIndex = targetArray.findIndex(tool => tool.id === dropTool.id);
      const newTargetArray = [...targetArray];
      newTargetArray.splice(dropIndex, 0, dragState.draggedTool);
      setTargetArray(newTargetArray);
      
      // Persist both arrays to store
      if (dragState.draggedFromBottom) {
        // Moving from bottom to top
        actions.editor.setToolbarBottomTabOrder(newSourceArray.map(tool => tool.id));
        actions.editor.setToolbarTabOrder(newTargetArray.map(tool => tool.id));
      } else {
        // Moving from top to bottom
        actions.editor.setToolbarTabOrder(newSourceArray.map(tool => tool.id));
        actions.editor.setToolbarBottomTabOrder(newTargetArray.map(tool => tool.id));
      }
    } else {
      // Handle reordering within the same section
      const draggedIndex = sourceArray.findIndex(tool => tool.id === dragState.draggedTool.id);
      const dropIndex = sourceArray.findIndex(tool => tool.id === dropTool.id);
      
      if (draggedIndex !== -1 && dropIndex !== -1 && draggedIndex !== dropIndex) {
        const newArray = [...sourceArray];
        const [removed] = newArray.splice(draggedIndex, 1);
        newArray.splice(dropIndex, 0, removed);
        setSourceArray(newArray);
        
        // Persist to store
        const newOrder = newArray.map(tool => tool.id);
        if (isBottomArea) {
          actions.editor.setToolbarBottomTabOrder(newOrder);
        } else {
          actions.editor.setToolbarTabOrder(newOrder);
        }
      }
    }

    setDragState({
      isDragging: false,
      draggedTool: null,
      dragOverTool: null,
      draggedFromBottom: false
    });
  };

  const handleDragEnd = () => {
    setDragState({
      isDragging: false,
      draggedTool: null,
      dragOverTool: null,
      draggedFromBottom: false
    });
  };

  const handleToolClick = (tool) => {
    if (!dragState.isDragging) {
      // Handle fullscreen functionality
      if (tool.id === 'fullscreen') {
        toggleFullscreen();
        return;
      }
      
      // Get current active viewport type for handling photo editor tools
      const currentWorkflow = getCurrentWorkflow();
      
      // Photo editor tools are now handled by the top horizontal toolbar
      // This toolbar only handles property panels for photo editor
      
      if (!scenePanelOpen) {
        onScenePanelToggle();
      }
      onToolSelect(tool.id);
    }
  };

  const toggleFullscreen = () => {
    if (!document.fullscreenElement) {
      // Enter fullscreen
      document.documentElement.requestFullscreen().catch(err => {
        console.warn('Error attempting to enable fullscreen:', err);
      });
    } else {
      // Exit fullscreen
      document.exitFullscreen().catch(err => {
        console.warn('Error attempting to exit fullscreen:', err);
      });
    }
  };


  return (
    <div className="relative w-12 h-full bg-gradient-to-b from-slate-800/95 to-slate-900/98 backdrop-blur-md border-l border-slate-700/80 shadow-2xl shadow-black/30 flex flex-col py-2 pointer-events-auto no-select">
      {/* Tools - made icons bigger */}
      <div className="flex flex-col space-y-1 px-1">
        {tools.map((tool) => {
          const isDragged = dragState.draggedTool?.id === tool.id;
          const isDragOver = dragState.dragOverTool?.id === tool.id;
          
          return (
            <button
              key={tool.id}
              draggable
              onClick={() => handleToolClick(tool)}
              onDragStart={(e) => handleDragStart(e, tool, false)}
              onDragOver={(e) => handleDragOver(e, tool, false)}
              onDragLeave={handleDragLeave}
              onDrop={(e) => handleDrop(e, tool, false)}
              onDragEnd={handleDragEnd}
              className={`p-2 rounded-lg transition-all duration-200 group relative select-none ${
                isDragged 
                  ? 'opacity-50 cursor-grabbing scale-95' 
                  : selectedTool === tool.id 
                    ? 'bg-gradient-to-b from-blue-500 to-blue-700 text-white shadow-lg shadow-blue-600/40 scale-105 cursor-grab' 
                    : 'text-slate-400 hover:text-white hover:bg-gradient-to-b hover:from-slate-700/80 hover:to-slate-800/90 hover:shadow-md hover:shadow-black/30 hover:scale-102 cursor-grab'
              }`}
              title={tool.title}
            >
              <tool.icon className="w-6 h-6" />
              
              {/* Drop indicator */}
              {isDragOver && (
                <div className="absolute inset-x-0 top-0 h-0.5 bg-blue-500 rounded-full"></div>
              )}
              
              {/* Tooltip with border and arrow - using higher z-index and better positioning */}
              {!dragState.isDragging && (
                <div className={`absolute ${shouldTooltipGoRight ? 'left-full ml-1' : 'right-full mr-1'} top-1/2 -translate-y-1/2 bg-slate-900/95 backdrop-blur-sm border border-slate-600 text-white text-xs px-3 py-1.5 rounded-md opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap shadow-2xl`} 
                     style={{ zIndex: 999999 }}>
                  {tool.title}
                  {/* Arrow pointing to the button */}
                  <div className={`absolute ${shouldTooltipGoRight ? 'right-full' : 'left-full'} top-1/2 -translate-y-1/2 w-0 h-0 ${shouldTooltipGoRight ? 'border-r-4 border-r-slate-900' : 'border-l-4 border-l-slate-900'} border-t-4 border-t-transparent border-b-4 border-b-transparent`}></div>
                </div>
              )}
            </button>
          );
        })}
      </div>
      
      {/* Spacer with drop zone */}
      <div 
        className="flex-1 flex items-center justify-center"
        onDragOver={(e) => {
          e.preventDefault();
          if (dragState.draggedTool) {
            e.dataTransfer.dropEffect = 'move';
          }
        }}
        onDrop={(e) => {
          e.preventDefault();
          e.stopPropagation();
          
          if (dragState.draggedTool) {
            const sourceArray = dragState.draggedFromBottom ? bottomTools : tools;
            const setSourceArray = dragState.draggedFromBottom ? setBottomTools : setTools;
            const targetArray = dragState.draggedFromBottom ? tools : bottomTools;
            const setTargetArray = dragState.draggedFromBottom ? setTools : setBottomTools;
            
            // Remove from source and add to target
            const newSourceArray = sourceArray.filter(tool => tool.id !== dragState.draggedTool.id);
            setSourceArray(newSourceArray);
            
            const newTargetArray = [...targetArray, dragState.draggedTool];
            setTargetArray(newTargetArray);
            
            // Persist to store
            if (dragState.draggedFromBottom) {
              // Moving from bottom to top
              actions.editor.setToolbarBottomTabOrder(newSourceArray.map(tool => tool.id));
              actions.editor.setToolbarTabOrder(newTargetArray.map(tool => tool.id));
            } else {
              // Moving from top to bottom
              actions.editor.setToolbarTabOrder(newSourceArray.map(tool => tool.id));
              actions.editor.setToolbarBottomTabOrder(newTargetArray.map(tool => tool.id));
            }
            
            setDragState({
              isDragging: false,
              draggedTool: null,
              dragOverTool: null,
              draggedFromBottom: false
            });
          }
        }}
      >
        {dragState.isDragging && (
          <div className="w-8 h-0.5 bg-blue-500/50 rounded-full opacity-50 transition-opacity">
          </div>
        )}
      </div>
      
      {/* Bottom Tools */}
      <div className="flex flex-col space-y-1 px-1">
        {bottomTools.map((tool) => {
          const isDragged = dragState.draggedTool?.id === tool.id;
          const isDragOver = dragState.dragOverTool?.id === tool.id;
          
          return (
            <button
              key={tool.id}
              draggable
              onClick={() => handleToolClick(tool)}
              onDragStart={(e) => handleDragStart(e, tool, true)}
              onDragOver={(e) => handleDragOver(e, tool, true)}
              onDragLeave={handleDragLeave}
              onDrop={(e) => handleDrop(e, tool, true)}
              onDragEnd={handleDragEnd}
              className={`p-2 rounded-lg transition-all duration-200 group relative select-none ${
                isDragged 
                  ? 'opacity-50 cursor-grabbing scale-95' 
                  : selectedTool === tool.id
                    ? 'bg-gradient-to-b from-blue-500 to-blue-700 text-white shadow-lg shadow-blue-600/40 scale-105 cursor-grab' 
                    : 'text-slate-400 hover:text-white hover:bg-gradient-to-b hover:from-slate-700/80 hover:to-slate-800/90 hover:shadow-md hover:shadow-black/30 hover:scale-102 cursor-grab'
              }`}
              title={tool.title}
            >
              <tool.icon className="w-6 h-6" />
              
              {/* Drop indicator */}
              {isDragOver && (
                <div className="absolute inset-x-0 top-0 h-0.5 bg-blue-500 rounded-full"></div>
              )}
              
              {/* Tooltip with border and arrow - using higher z-index and better positioning */}
              {!dragState.isDragging && (
                <div className={`absolute ${shouldTooltipGoRight ? 'left-full ml-1' : 'right-full mr-1'} top-1/2 -translate-y-1/2 bg-slate-900/95 backdrop-blur-sm border border-slate-600 text-white text-xs px-3 py-1.5 rounded-md opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap shadow-2xl`} 
                     style={{ zIndex: 999999 }}>
                  {tool.title}
                  {/* Arrow pointing to the button */}
                  <div className={`absolute ${shouldTooltipGoRight ? 'right-full' : 'left-full'} top-1/2 -translate-y-1/2 w-0 h-0 ${shouldTooltipGoRight ? 'border-r-4 border-r-slate-900' : 'border-l-4 border-l-slate-900'} border-t-4 border-t-transparent border-b-4 border-b-transparent`}></div>
                </div>
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
}

export default Toolbar;