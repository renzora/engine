import React, { useState } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';
import { useSnapshot } from 'valtio';
import { globalStore, actions } from "@/store.js";

const ViewportTabs = () => {
  const [isAddDropdownOpen, setIsAddDropdownOpen] = useState(false);
  const [dropdownPosition, setDropdownPosition] = useState({ x: 0, y: 0 });
  const [contextMenu, setContextMenu] = useState(null);
  const [editingTab, setEditingTab] = useState(null);
  const [editingName, setEditingName] = useState('');
  const { tabs, activeTabId, suspendedTabs } = useSnapshot(globalStore.editor.viewport);
  
  const {
    addViewportTab,
    setActiveViewportTab,
    closeViewportTab,
    pinViewportTab,
    duplicateViewportTab,
    updateViewportTab,
    renameViewportTab
  } = actions.editor;

  const availableViewportTypes = [
    {
      id: '3d-viewport',
      label: 'New Scene',
      icon: Icons.Cube,
      description: 'Create a new 3D scene viewport'
    },
    {
      id: 'node-editor',
      label: 'Node Editor',
      icon: Icons.Cog,
      description: 'Create a new node editor for visual scripting'
    }
  ];

  const getViewportIcon = (type) => {
    const viewportType = availableViewportTypes.find(v => v.id === type);
    return viewportType ? viewportType.icon : Icons.FileText;
  };

  const handleAddViewport = (type) => {
    addViewportTab(type);
    setIsAddDropdownOpen(false);
  };

  const handleTabClick = (tabId) => {
    setActiveViewportTab(tabId);
  };

  const handleTabClose = (e, tabId) => {
    e.stopPropagation();
    closeViewportTab(tabId);
  };

  const handleStartRename = (tab) => {
    setEditingTab(tab.id);
    setEditingName(tab.name);
    setContextMenu(null);
  };

  const handleFinishRename = () => {
    if (editingTab && editingName.trim()) {
      renameViewportTab(editingTab, editingName.trim());
    }
    setEditingTab(null);
    setEditingName('');
  };

  const handleCancelRename = () => {
    setEditingTab(null);
    setEditingName('');
  };

  const handleRenameKeyDown = (e) => {
    if (e.key === 'Enter') {
      handleFinishRename();
    } else if (e.key === 'Escape') {
      handleCancelRename();
    }
  };

  const handleTabContextMenu = (e, tab) => {
    e.preventDefault();
    e.stopPropagation();
    
    setContextMenu({
      position: { x: e.clientX, y: e.clientY },
      tab: tab,
      items: [
        {
          label: 'Rename Tab',
          icon: Icons.FileText,
          action: () => handleStartRename(tab)
        },
        {
          label: tab.isPinned ? 'Unpin Tab' : 'Pin Tab',
          icon: tab.isPinned ? Icons.X : Icons.Star,
          action: () => pinViewportTab(tab.id)
        },
        {
          label: 'Duplicate Tab',
          icon: Icons.Copy,
          action: () => duplicateViewportTab(tab.id)
        },
        {
          label: (suspendedTabs || []).includes(tab.id) ? 'Resume Tab' : 'Suspend Tab',
          icon: (suspendedTabs || []).includes(tab.id) ? Icons.Play : Icons.Pause,
          action: () => {
            if ((suspendedTabs || []).includes(tab.id)) {
              actions.editor.resumeTab(tab.id);
            } else {
              actions.editor.suspendTab(tab.id);
            }
          },
          disabled: tab.id === activeTabId
        },
        { divider: true },
        {
          label: `New ${tab.type === '3d-viewport' ? 'Scene' : tab.type.replace('-', ' ').replace(/\b\w/g, l => l.toUpperCase())}`,
          icon: getViewportIcon(tab.type),
          action: () => addViewportTab(tab.type)
        },
        { divider: true },
        {
          label: 'Close Tab',
          icon: Icons.X,
          action: () => closeViewportTab(tab.id),
          disabled: tabs.length === 1
        },
        {
          label: 'Close Other Tabs',
          icon: Icons.XMark,
          action: () => {
            tabs.forEach(t => {
              if (t.id !== tab.id && !t.isPinned) {
                closeViewportTab(t.id);
              }
            });
          },
          disabled: tabs.length === 1
        },
        {
          label: 'Close All Tabs',
          icon: Icons.XMark,
          action: () => {
            tabs.forEach(t => {
              if (!t.isPinned) {
                closeViewportTab(t.id);
              }
            });
          },
          disabled: tabs.length === 1 || tabs.every(t => t.isPinned)
        }
      ]
    });
  };

  const handleMiddleClick = (e, tabId) => {
    if (e.button === 1) {
      e.preventDefault();
      closeViewportTab(tabId);
    }
  };

  return (
    <>
      <div className="flex items-center h-8 bg-gray-900/95 border-b border-gray-800">
        <div className="flex items-center min-w-0 flex-1 overflow-x-auto">
          {tabs.map((tab) => {
            const Icon = getViewportIcon(tab.type);
            const isActive = tab.id === activeTabId;
            
            return (
              <div
                key={tab.id}
                className={`group flex items-center gap-2 px-3 py-1 border-r border-gray-700 cursor-pointer transition-all select-none min-w-0 max-w-48 flex-shrink-0 ${
                  isActive
                    ? 'bg-blue-600/20 border-b-2 border-b-blue-500 text-blue-300'
                    : 'text-gray-400 hover:text-gray-200 hover:bg-gray-800'
                }`}
                onClick={() => handleTabClick(tab.id)}
                onContextMenu={(e) => handleTabContextMenu(e, tab)}
                onMouseDown={(e) => handleMiddleClick(e, tab.id)}
                title={tab.name}
              >
                <Icon className="w-4 h-4 flex-shrink-0" />
                
                {editingTab === tab.id ? (
                  <input
                    type="text"
                    value={editingName}
                    onChange={(e) => setEditingName(e.target.value)}
                    onBlur={handleFinishRename}
                    onKeyDown={handleRenameKeyDown}
                    className="text-sm font-medium bg-gray-700 border border-gray-500 rounded px-1 py-0 min-w-0 max-w-32"
                    autoFocus
                    onClick={(e) => e.stopPropagation()}
                  />
                ) : (
                  <span className="text-sm font-medium truncate min-w-0">
                    {tab.name}
                  </span>
                )}
                
                {(suspendedTabs || []).includes(tab.id) && (
                  <Icons.Pause className="w-3 h-3 text-gray-500 flex-shrink-0" title="Tab Suspended" />
                )}
                
                {tab.isPinned && (
                  <Icons.Star className="w-3 h-3 text-yellow-500 flex-shrink-0" />
                )}
                
                {tab.hasUnsavedChanges && (
                  <div className="w-2 h-2 bg-orange-500 rounded-full flex-shrink-0" />
                )}
                
                {tabs.length > 1 && (
                  <button
                    onClick={(e) => handleTabClose(e, tab.id)}
                    className="w-4 h-4 flex items-center justify-center rounded hover:bg-gray-600 transition-colors opacity-0 group-hover:opacity-100 flex-shrink-0"
                    title="Close Tab"
                  >
                    <Icons.X className="w-3 h-3" />
                  </button>
                )}
              </div>
            );
          })}

          <div className="relative flex-shrink-0">
            <button
              onClick={(e) => {
                const rect = e.currentTarget.getBoundingClientRect();
                setDropdownPosition({
                  x: Math.min(rect.left, window.innerWidth - 280),
                  y: rect.bottom + 4
                });
                setIsAddDropdownOpen(!isAddDropdownOpen);
              }}
              className="flex items-center px-3 py-1 text-gray-400 hover:text-gray-200 hover:bg-gray-800 transition-colors border-r border-gray-700"
              title="Add Viewport"
            >
              <Icons.Plus className="w-4 h-4" />
            </button>

            {isAddDropdownOpen && (
              <>
                <div 
                  className="fixed inset-0 z-40"
                  onClick={() => setIsAddDropdownOpen(false)}
                />
                
                <div 
                  className="fixed w-64 bg-gray-900/98 backdrop-blur-sm border border-gray-700/50 rounded-lg shadow-xl z-50"
                  style={{
                    left: dropdownPosition.x,
                    top: dropdownPosition.y
                  }}
                >
                  <div className="p-2">
                    <div className="text-xs text-gray-500 uppercase tracking-wide px-2 py-1 mb-1">
                      Add Viewport
                    </div>
                    {availableViewportTypes.map((viewportType) => (
                      <button
                        key={viewportType.id}
                        onClick={() => handleAddViewport(viewportType.id)}
                        className="w-full flex items-start px-3 py-2 text-sm text-gray-300 hover:bg-gray-700/50 hover:text-white rounded-md transition-colors group"
                      >
                        <viewportType.icon className="w-4 h-4 mr-3 mt-0.5 text-gray-400 group-hover:text-white flex-shrink-0" />
                        <div className="text-left min-w-0">
                          <div className="font-medium">{viewportType.label}</div>
                          <div className="text-xs text-gray-500 group-hover:text-gray-300">
                            {viewportType.description}
                          </div>
                        </div>
                      </button>
                    ))}
                  </div>
                </div>
              </>
            )}
          </div>
        </div>

        <div className="flex items-center gap-1 px-2 border-l border-gray-700 flex-shrink-0">
          <button
            className="p-1 text-gray-400 hover:text-gray-200 hover:bg-gray-800 rounded transition-colors"
            title="Split Viewport"
          >
            <Icons.Grid className="w-4 h-4" />
          </button>
          <button
            className="p-1 text-gray-400 hover:text-gray-200 hover:bg-gray-800 rounded transition-colors"
            title="Viewport Settings"
          >
            <Icons.Settings className="w-4 h-4" />
          </button>
        </div>
      </div>

      {contextMenu && (
        <>
          <div 
            className="fixed inset-0 z-50"
            onClick={() => setContextMenu(null)}
          />
          
          <div
            className="fixed z-60 bg-gray-900/98 backdrop-blur-sm border border-gray-700/50 rounded-lg shadow-xl min-w-48"
            style={{ 
              left: contextMenu.position.x, 
              top: contextMenu.position.y 
            }}
          >
            <div className="p-2">
              {contextMenu.items.map((item, index) => (
                item.divider ? (
                  <div key={index} className="border-t border-gray-700/50 my-2" />
                ) : (
                  <button
                    key={index}
                    onClick={() => {
                      if (!item.disabled) {
                        item.action();
                      }
                      setContextMenu(null);
                    }}
                    disabled={item.disabled}
                    className={`w-full flex items-center px-3 py-2 text-sm rounded-md transition-colors ${
                      item.disabled 
                        ? 'text-gray-600 cursor-not-allowed'
                        : 'text-gray-300 hover:bg-gray-700/50 hover:text-white'
                    }`}
                  >
                    {item.icon && (
                      <item.icon className={`w-4 h-4 mr-3 ${
                        item.disabled ? 'text-gray-600' : 'text-gray-400'
                      }`} />
                    )}
                    {item.label}
                  </button>
                )
              ))}
            </div>
          </div>
        </>
      )}
    </>
  );
};

export default ViewportTabs;