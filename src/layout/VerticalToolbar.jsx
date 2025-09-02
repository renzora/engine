import { createSignal, createEffect, createMemo, For } from 'solid-js';
import { Cube, Plus, Settings, Maximize } from '@/ui/icons';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { viewportStore } from '@/layout/stores/ViewportStore';
import { propertyTabs, toolbarButtons } from '@/api/plugin';

const defaultTools = [
  { id: 'scene', icon: Cube, title: 'Scene' }
];

const defaultBottomTools = [];

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

function Toolbar(props) {
  const [tools, setTools] = createSignal(() => getOrderedTools());
  
  createEffect(() => {
    propertyTabs();
    toolbarButtons();
    const newTools = getOrderedTools();
    setTools(newTools);
  });
  const [bottomTools, setBottomTools] = createSignal(() => getOrderedBottomTools());
  
  const [dragState, setDragState] = createSignal({
    isDragging: false,
    draggedTool: null,
    dragOverTool: null,
    draggedFromBottom: false
  });

  const settings = createMemo(() => editorStore.settings);
  const panelPosition = createMemo(() => settings().editor.panelPosition || 'right');
  const isPanelOnLeft = createMemo(() => panelPosition() === 'left');
  const shouldTooltipGoRight = createMemo(() => isPanelOnLeft());

  const viewport = createMemo(() => viewportStore);
  const ui = createMemo(() => editorStore.ui);
  const getCurrentWorkflow = () => {
    const viewportData = viewport();
    if (!viewportData.tabs || viewportData.tabs.length === 0) {
      return 'default';
    }
    const activeTabData = viewportData.tabs.find(tab => tab.id === viewportData.activeTabId);
    return activeTabData?.type || 'default';
  };
  
  function getOrderedTools() {
    const currentWorkflow = getCurrentWorkflow();
    const allowedToolIds = workflowTools[currentWorkflow] || workflowTools['default'];
    const pluginTabs = Array.from(propertyTabs().values())
      .sort((a, b) => (a.order || 0) - (b.order || 0))
      .map(tab => ({
        id: tab.id,
        icon: tab.icon,
        title: tab.title
      }));
    
    const pluginButtons = Array.from(toolbarButtons().values())
      .filter(button => button.section === 'main')
      .sort((a, b) => (a.order || 0) - (b.order || 0))
      .map(button => ({
        id: button.id,
        icon: button.icon,
        title: button.title,
        onClick: button.onClick,
        isPluginButton: true
      }));
    
    const allTools = [...defaultTools, ...pluginTabs, ...pluginButtons];
    
    const toolsMap = allTools.reduce((map, tool) => {
      map[tool.id] = tool;
      return map;
    }, {});
    
    let currentTabOrder = ui().toolbarTabOrder || [];
    const missingTools = allTools
      .filter(tool => !currentTabOrder.includes(tool.id))
      .map(tool => tool.id);
    
    if (missingTools.length > 0) {
      currentTabOrder = [...currentTabOrder, ...missingTools];
      editorActions.setToolbarTabOrder(currentTabOrder);
    }
    
    if (!currentTabOrder || !Array.isArray(currentTabOrder)) {
      return allTools.filter(tool => 
        allowedToolIds.includes(tool.id) || 
        propertyTabs().has(tool.id)
      );
    }
    
    const workflowFilteredTools = currentTabOrder
      .filter(id => allowedToolIds.includes(id) || propertyTabs().has(id))
      .map(id => toolsMap[id])
      .filter(Boolean);
    
    return workflowFilteredTools;
  }
  
  function getOrderedBottomTools() {
    const pluginBottomButtons = Array.from(toolbarButtons().values())
      .filter(button => button.section === 'bottom')
      .sort((a, b) => (a.order || 0) - (b.order || 0))
      .map(button => ({
        id: button.id,
        icon: button.icon,
        title: button.title,
        onClick: button.onClick,
        isPluginButton: true
      }));

    const allBottomTools = [...defaultBottomTools, ...pluginBottomButtons];
    
    const toolbarBottomTabOrder = ui().toolbarBottomTabOrder;
    if (!toolbarBottomTabOrder || !Array.isArray(toolbarBottomTabOrder)) {
      return allBottomTools;
    }
    const toolsMap = allBottomTools.reduce((map, tool) => {
      map[tool.id] = tool;
      return map;
    }, {});
    
    const orderedTools = toolbarBottomTabOrder.map(id => toolsMap[id]).filter(Boolean);
    const existingIds = new Set(toolbarBottomTabOrder);
    const newTools = allBottomTools.filter(tool => !existingIds.has(tool.id));
    
    return [...orderedTools, ...newTools];
  }

  createEffect(() => {
    const orderedTools = getOrderedTools();
    setTools(orderedTools);
  });
  
  createEffect(() => {
    toolbarButtons();
    const newBottomTools = getOrderedBottomTools();
    setBottomTools(newBottomTools);
  });

  const handleDragStart = (e, tool, isFromBottom = false) => {
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/html', '');
    
    const dragElement = e.currentTarget.cloneNode(true);
    dragElement.style.position = 'absolute';
    dragElement.style.top = '-1000px';
    dragElement.style.left = '-1000px';
    dragElement.style.background = 'oklch(var(--b2) / 0.95)';
    dragElement.style.border = '1px solid oklch(var(--b3) / 0.5)';
    dragElement.style.borderRadius = '8px';
    dragElement.style.padding = '8px';
    dragElement.style.boxShadow = '0 25px 50px -12px rgb(0 0 0 / 0.5)';
    dragElement.style.transform = 'scale(1.1)';
    dragElement.style.pointerEvents = 'none';
    dragElement.style.zIndex = '9999';
    
    const icon = dragElement.querySelector('svg');
    if (icon) {
      icon.style.color = 'oklch(var(--bc))';
    }
    
    document.body.appendChild(dragElement);
    e.dataTransfer.setDragImage(dragElement, 24, 24);
    
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
    
    if (dragState().draggedTool && dragState().draggedTool.id !== tool.id) {
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
    
    const currentDragState = dragState();
    if (!currentDragState.draggedTool || currentDragState.draggedTool.id === dropTool.id) {
      setDragState({
        isDragging: false,
        draggedTool: null,
        dragOverTool: null,
        draggedFromBottom: false
      });
      return;
    }

    const sourceArray = currentDragState.draggedFromBottom ? bottomTools() : tools();
    const targetArray = isBottomArea ? bottomTools() : tools();
    const setSourceArray = currentDragState.draggedFromBottom ? setBottomTools : setTools;
    const setTargetArray = isBottomArea ? setBottomTools : setTools;

    if (currentDragState.draggedFromBottom !== isBottomArea) {
      const newSourceArray = sourceArray.filter(tool => tool.id !== currentDragState.draggedTool.id);
      setSourceArray(newSourceArray);
      
      const dropIndex = targetArray.findIndex(tool => tool.id === dropTool.id);
      const newTargetArray = [...targetArray];
      newTargetArray.splice(dropIndex, 0, currentDragState.draggedTool);
      setTargetArray(newTargetArray);
      
      if (currentDragState.draggedFromBottom) {
        editorActions.setToolbarBottomTabOrder(newSourceArray.map(tool => tool.id));
        editorActions.setToolbarTabOrder(newTargetArray.map(tool => tool.id));
      } else {
        editorActions.setToolbarTabOrder(newSourceArray.map(tool => tool.id));
        editorActions.setToolbarBottomTabOrder(newTargetArray.map(tool => tool.id));
      }
    } else {
      const draggedIndex = sourceArray.findIndex(tool => tool.id === currentDragState.draggedTool.id);
      const dropIndex = sourceArray.findIndex(tool => tool.id === dropTool.id);
      
      if (draggedIndex !== -1 && dropIndex !== -1 && draggedIndex !== dropIndex) {
        const newArray = [...sourceArray];
        const [removed] = newArray.splice(draggedIndex, 1);
        newArray.splice(dropIndex, 0, removed);
        setSourceArray(newArray);
        
        const newOrder = newArray.map(tool => tool.id);
        if (isBottomArea) {
          editorActions.setToolbarBottomTabOrder(newOrder);
        } else {
          editorActions.setToolbarTabOrder(newOrder);
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
    if (!dragState().isDragging) {
      if (tool.isPluginButton && tool.onClick) {
        tool.onClick();
        return;
      }
      
      const currentWorkflow = getCurrentWorkflow();
      
      if (!props.scenePanelOpen) {
        props.onScenePanelToggle();
      }
      props.onToolSelect(tool.id);
    }
  };


  return (
    <div class="relative w-10 h-full bg-base-300 border-l border-t border-r border-base-content/10 flex flex-col pointer-events-auto no-select">
      <div class="flex flex-col space-y-0.5">
        <For each={tools()}>
          {(tool) => {
            const isDragged = () => dragState().draggedTool?.id === tool.id;
            const isDragOver = () => dragState().dragOverTool?.id === tool.id;
            
            return (
              <button
                draggable
                onClick={() => handleToolClick(tool)}
                onDragStart={(e) => handleDragStart(e, tool, false)}
                onDragOver={(e) => handleDragOver(e, tool, false)}
                onDragLeave={handleDragLeave}
                onDrop={(e) => handleDrop(e, tool, false)}
                onDragEnd={handleDragEnd}
                class={`p-1.5 transition-all duration-200 group relative select-none w-full flex items-center justify-center ${
                  isDragged() 
                    ? 'opacity-50 cursor-grabbing scale-95' 
                    : props.selectedTool === tool.id 
                      ? 'bg-primary text-primary-content cursor-grab' 
                      : 'text-base-content/60 hover:text-base-content hover:bg-base-200 cursor-grab'
                }`}
                title={tool.title}
              >
                <tool.icon class="w-5 h-5" />
                
                {isDragOver() && (
                  <div class="absolute inset-x-0 top-0 h-0.5 bg-primary rounded-full"></div>
                )}
                
                {!dragState().isDragging && (
                  <div class={`absolute ${shouldTooltipGoRight() ? 'left-full ml-1' : 'right-full mr-1'} top-1/2 -translate-y-1/2 bg-base-300/95 backdrop-blur-sm border border-base-300 text-base-content text-xs px-3 py-1.5 rounded-md opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap shadow-2xl`} 
                       style={{ 'z-index': 999999 }}>
                    {tool.title}
                    <div class={`absolute ${shouldTooltipGoRight() ? 'right-full' : 'left-full'} top-1/2 -translate-y-1/2 w-0 h-0 ${shouldTooltipGoRight() ? 'border-r-4 border-r-base-300' : 'border-l-4 border-l-base-300'} border-t-4 border-t-transparent border-b-4 border-b-transparent`}></div>
                  </div>
                )}
              </button>
            );
          }}
        </For>
      </div>
      
      <div 
        class="flex-1 flex items-center justify-center cursor-col-resize"
        onMouseDown={(e) => {
          if (!props.panelResize) return;
          e.preventDefault();
          props.panelResize.handleRightResizeStart(e);
          
          const handleMouseMove = (e) => {
            e.preventDefault();
            props.panelResize.handleRightResizeMove(e, { 
              isScenePanelOpen: typeof props.scenePanelOpen === 'function' ? props.scenePanelOpen : () => props.scenePanelOpen,
              isLeftPanel: props.isLeftPanel,
              selectedRightTool: props.selectedTool
            });
          };

          const handleMouseUp = (e) => {
            e.preventDefault();
            props.panelResize.handleRightResizeEnd();
            document.removeEventListener('mousemove', handleMouseMove);
            document.removeEventListener('mouseup', handleMouseUp);
          };

          document.addEventListener('mousemove', handleMouseMove);
          document.addEventListener('mouseup', handleMouseUp);
        }}
        onDragOver={(e) => {
          e.preventDefault();
          if (dragState().draggedTool) {
            e.dataTransfer.dropEffect = 'move';
          }
        }}
        onDrop={(e) => {
          e.preventDefault();
          e.stopPropagation();
          
          const currentDragState = dragState();
          if (currentDragState.draggedTool) {
            const sourceArray = currentDragState.draggedFromBottom ? bottomTools() : tools();
            const setSourceArray = currentDragState.draggedFromBottom ? setBottomTools : setTools;
            const targetArray = currentDragState.draggedFromBottom ? tools() : bottomTools();
            const setTargetArray = currentDragState.draggedFromBottom ? setTools : setBottomTools;
            const newSourceArray = sourceArray.filter(tool => tool.id !== currentDragState.draggedTool.id);
            setSourceArray(newSourceArray);
            
            const newTargetArray = [...targetArray, currentDragState.draggedTool];
            setTargetArray(newTargetArray);
            
            if (currentDragState.draggedFromBottom) {
              editorActions.setToolbarBottomTabOrder(newSourceArray.map(tool => tool.id));
              editorActions.setToolbarTabOrder(newTargetArray.map(tool => tool.id));
            } else {
              editorActions.setToolbarTabOrder(newSourceArray.map(tool => tool.id));
              editorActions.setToolbarBottomTabOrder(newTargetArray.map(tool => tool.id));
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
        {dragState().isDragging && (
          <div class="w-8 h-0.5 bg-primary/50 rounded-full opacity-50 transition-opacity">
          </div>
        )}
      </div>
      
      <div class="flex flex-col space-y-0.5">
        <For each={bottomTools()}>
          {(tool) => {
            const isDragged = () => dragState().draggedTool?.id === tool.id;
            const isDragOver = () => dragState().dragOverTool?.id === tool.id;
            
            return (
              <button
                draggable
                onClick={() => handleToolClick(tool)}
                onDragStart={(e) => handleDragStart(e, tool, true)}
                onDragOver={(e) => handleDragOver(e, tool, true)}
                onDragLeave={handleDragLeave}
                onDrop={(e) => handleDrop(e, tool, true)}
                onDragEnd={handleDragEnd}
                class={`p-1.5 transition-all duration-200 group relative select-none w-full flex items-center justify-center ${
                  isDragged() 
                    ? 'opacity-50 cursor-grabbing scale-95' 
                    : props.selectedTool === tool.id
                      ? 'bg-primary text-primary-content cursor-grab' 
                      : 'text-base-content/60 hover:text-base-content hover:bg-base-200 cursor-grab'
                }`}
                title={tool.title}
              >
                <tool.icon class="w-5 h-5" />
                
                {isDragOver() && (
                  <div class="absolute inset-x-0 top-0 h-0.5 bg-primary rounded-full"></div>
                )}
                
                {!dragState().isDragging && (
                  <div class={`absolute ${shouldTooltipGoRight() ? 'left-full ml-1' : 'right-full mr-1'} top-1/2 -translate-y-1/2 bg-base-300/95 backdrop-blur-sm border border-base-300 text-base-content text-xs px-3 py-1.5 rounded-md opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap shadow-2xl`} 
                       style={{ 'z-index': 999999 }}>
                    {tool.title}
                    <div class={`absolute ${shouldTooltipGoRight() ? 'right-full' : 'left-full'} top-1/2 -translate-y-1/2 w-0 h-0 ${shouldTooltipGoRight() ? 'border-r-4 border-r-base-300' : 'border-l-4 border-l-base-300'} border-t-4 border-t-transparent border-b-4 border-b-transparent`}></div>
                  </div>
                )}
              </button>
            );
          }}
        </For>
      </div>
    </div>
  );
}

export default Toolbar;
