import React, { useState } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';
import { useSnapshot } from 'valtio';
import { globalStore, actions } from "@/store.js";

function WorkflowTabs() {
  const [activeWorkflow, setActiveWorkflow] = useState('modeling');
  const { ui } = useSnapshot(globalStore.editor);

  const workflows = {
    modeling: {
      name: 'Modeling',
      icon: Icons.Cube || Icons.Mesh,
      description: 'Create and edit 3D models',
      color: 'blue',
      panels: ['scene', 'properties', 'assets'],
      tools: ['select', 'move', 'rotate', 'scale', 'extrude', 'inset']
    },
    sculpting: {
      name: 'Sculpting',
      icon: Icons.Paintbrush2 || Icons.PaintBrush,
      description: 'Sculpt organic shapes and details',
      color: 'purple',
      panels: ['scene', 'properties', 'brushes'],
      tools: ['grab', 'smooth', 'inflate', 'pinch', 'crease']
    },
    shading: {
      name: 'Shading',
      icon: Icons.Palette || Icons.ColorSwatch,
      description: 'Create and edit materials',
      color: 'green',
      panels: ['scene', 'shader-editor', 'assets'],
      tools: ['material-picker', 'node-editor', 'texture-paint']
    },
    animation: {
      name: 'Animation',
      icon: Icons.Play,
      description: 'Animate objects and characters',
      color: 'orange',
      panels: ['scene', 'timeline', 'dope-sheet'],
      tools: ['keyframe', 'timeline', 'graph-editor', 'nla-editor']
    },
    rendering: {
      name: 'Rendering',
      icon: Icons.Video,
      description: 'Set up cameras and lighting',
      color: 'red',
      panels: ['scene', 'render-properties', 'compositor'],
      tools: ['camera', 'light', 'world-settings', 'render-layers']
    },
  };

  const handleWorkflowChange = (workflowId) => {
    setActiveWorkflow(workflowId);
    const workflow = workflows[workflowId];
    actions.editor.setWorkflowMode(workflowId);
    actions.editor.addConsoleMessage(`Switched to ${workflow.name} workflow`, 'info');
    console.log(`Switched to ${workflow.name} workflow`);
    console.log('Recommended panels:', workflow.panels);
    console.log('Available tools:', workflow.tools);
  };

  const getColorClasses = (color, isActive) => {
    const colors = {
      blue: isActive ? 'bg-blue-600/90 text-white border-blue-500' : 'hover:bg-blue-600/20 hover:border-blue-500/50',
      purple: isActive ? 'bg-purple-600/90 text-white border-purple-500' : 'hover:bg-purple-600/20 hover:border-purple-500/50',
      green: isActive ? 'bg-green-600/90 text-white border-green-500' : 'hover:bg-green-600/20 hover:border-green-500/50',
      orange: isActive ? 'bg-orange-600/90 text-white border-orange-500' : 'hover:bg-orange-600/20 hover:border-orange-500/50',
      red: isActive ? 'bg-red-600/90 text-white border-red-500' : 'hover:bg-red-600/20 hover:border-red-500/50',
      yellow: isActive ? 'bg-yellow-600/90 text-white border-yellow-500' : 'hover:bg-yellow-600/20 hover:border-yellow-500/50',
    };
    return colors[color] || colors.blue;
  };

  return (
    <div className="relative w-full h-9 bg-gray-800/95 backdrop-blur-sm border-b border-gray-700 flex items-center">
      <div className="flex items-center h-full w-full px-4">
        
        <div className="flex items-center">
          {Object.entries(workflows).map(([workflowId, workflow]) => {
            const isActive = activeWorkflow === workflowId;
            return (
              <button
                key={workflowId}
                onClick={() => handleWorkflowChange(workflowId)}
                className={`relative flex items-center gap-2 px-3 py-1.5 text-sm font-medium transition-all whitespace-nowrap group ${
                  isActive 
                    ? 'text-blue-400' 
                    : 'text-gray-400 hover:text-gray-200 hover:bg-slate-800'
                }`}
                title={workflow.description}
              >
                <workflow.icon className="w-4 h-4" />
                <span>{workflow.name}</span>
                
                {isActive && (
                  <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-blue-500" />
                )}

                {!isActive && (
                  <div className="absolute bottom-full mb-2 left-1/2 transform -translate-x-1/2 bg-gray-900/95 text-white text-xs px-3 py-2 rounded-lg whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50 max-w-xs">
                    <div className="font-medium">{workflow.name}</div>
                    <div className="text-gray-300 mt-1">{workflow.description}</div>
                    <div className="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-gray-900/95" />
                  </div>
                )}
              </button>
            );
          })}
        </div>

        <div className="flex-1" />

        <div className="flex items-center gap-2">
          {activeWorkflow === 'modeling' && (
            <>
              <button className="px-2 py-1 text-xs text-gray-400 hover:text-gray-200 hover:bg-slate-800 rounded transition-colors">
                Subdivision
              </button>
              <button className="px-2 py-1 text-xs text-gray-400 hover:text-gray-200 hover:bg-slate-800 rounded transition-colors">
                Mirror
              </button>
            </>
          )}
          
          {activeWorkflow === 'animation' && (
            <>
              <button className="px-2 py-1 text-xs text-gray-400 hover:text-gray-200 hover:bg-slate-800 rounded transition-colors">
                Auto Key
              </button>
              <button className="px-2 py-1 text-xs text-gray-400 hover:text-gray-200 hover:bg-slate-800 rounded transition-colors">
                Onion Skin
              </button>
            </>
          )}
          
          {activeWorkflow === 'rendering' && (
            <>
              <button className="px-2 py-1 text-xs text-gray-400 hover:text-gray-200 hover:bg-slate-800 rounded transition-colors">
                Render
              </button>
              <button className="px-2 py-1 text-xs text-gray-400 hover:text-gray-200 hover:bg-slate-800 rounded transition-colors">
                Preview
              </button>
            </>
          )}

          <button className="w-7 h-7 flex items-center justify-center text-gray-400 hover:text-gray-200 hover:bg-slate-800 rounded transition-colors">
            {Icons.Settings ? <Icons.Settings className="w-4 h-4" /> : <Icons.Cog className="w-4 h-4" />}
          </button>
        </div>
      </div>
    </div>
  );
}

export default WorkflowTabs;