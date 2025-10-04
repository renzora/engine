import { createSignal, createContext, useContext, onCleanup } from 'solid-js';
import ContextMenu from './ContextMenu.jsx';
import { createContextMenuActions } from './ContextMenuActions.jsx';

// Context for sharing context menu state across components
const ViewportContextMenuContext = createContext();

export function ViewportContextMenuProvider(props) {
  const [contextMenuState, setContextMenuState] = createSignal(null);
  const contextMenuActions = createContextMenuActions(props.editorActions);

  const showContextMenu = (event, item = null, context = 'viewport', currentPath = '') => {
    if (!event) return;
    
    event.preventDefault();
    event.stopPropagation();

    const { clientX: x, clientY: y } = event;
    const items = contextMenuActions.getContextMenuItems(item, context, currentPath);

    setContextMenuState({
      position: { x, y },
      items,
      visible: true
    });
  };

  const hideContextMenu = () => {
    setContextMenuState(null);
  };

  // Close context menu on escape key
  const handleKeyDown = (e) => {
    if (e.key === 'Escape') {
      hideContextMenu();
    }
  };

  // Add global event listener for escape key
  document.addEventListener('keydown', handleKeyDown);
  onCleanup(() => {
    document.removeEventListener('keydown', handleKeyDown);
  });

  const contextValue = {
    showContextMenu,
    hideContextMenu,
    contextMenuState: contextMenuState()
  };

  return (
    <ViewportContextMenuContext.Provider value={contextValue}>
      {props.children}
      {contextMenuState() && (
        <ContextMenu 
          items={contextMenuState().items}
          position={contextMenuState().position}
          onClose={hideContextMenu}
        />
      )}
    </ViewportContextMenuContext.Provider>
  );
}

export function useViewportContextMenu() {
  const context = useContext(ViewportContextMenuContext);
  if (!context) {
    throw new Error('useViewportContextMenu must be used within ViewportContextMenuProvider');
  }
  return context;
}

// Higher-order component to add context menu support to any component
export function withContextMenu(Component, contextType) {
  return function WrappedComponent(props) {
    const { showContextMenu } = useViewportContextMenu();
    
    const handleContextMenu = (event, item = null, currentPath = '') => {
      showContextMenu(event, item, contextType, currentPath);
    };

    return (
      <Component 
        {...props} 
        onContextMenu={handleContextMenu}
      />
    );
  };
}