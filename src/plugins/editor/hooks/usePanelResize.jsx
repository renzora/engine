import { createSignal, batch } from 'solid-js';

export const createPanelResize = (editorActions) => {
  const [isResizingBottom, setIsResizingBottom] = createSignal(false);
  const [isResizingRight, setIsResizingRight] = createSignal(false);
  let justOpenedPanel = false; // Flag to prevent immediate close after opening

  const {
    setBottomPanelHeight,
    setRightPanelWidth,
    setAssetPanelOpen,
    setScenePanelOpen,
    setResizingPanels,
    setSelectedTool: setSelectedRightTool
  } = editorActions;

  const handleBottomResizeStart = () => {
    console.log('🔵 Bottom panel resize START');
    setIsResizingBottom(true);
    setResizingPanels(true);
    document.body.classList.add('dragging-vertical');
    
  };

  const handleBottomResizeMove = (e, { isAssetPanelOpen }) => {
    if (!isResizingBottom()) return;
    e.preventDefault();
    
    const headerHeight = 48; // Height of the top toolbar/header
    const horizontalToolbarHeight = 48; // Height of the horizontal toolbar
    const viewportTabsHeight = 32; // Height of the viewport tabs
    const minViewportSpace = headerHeight + horizontalToolbarHeight + viewportTabsHeight; // Panel stops below viewport tabs
    const newHeight = window.innerHeight - e.clientY;
    const windowHeight = window.innerHeight;
    const maxPossibleHeight = windowHeight - minViewportSpace; // Can extend to just below horizontal toolbar
    const snapToMaxThreshold = maxPossibleHeight - 30; // Snap when close to max height
    const snapToCloseThreshold = 80;
    const openThreshold = 60; // Lower threshold for easier opening
    
    // Get the current state directly from the function
    const isPanelOpen = typeof isAssetPanelOpen === 'function' ? isAssetPanelOpen() : isAssetPanelOpen;
    
    console.log('🟢 Bottom resize move:', { 
      newHeight, 
      isPanelOpen, 
      clientY: e.clientY,
      snapCheck: newHeight > snapToMaxThreshold,
      maxPossibleHeight 
    });
    
    // Always update the height when dragging, regardless of open state
    const constrainedHeight = Math.max(40, Math.min(maxPossibleHeight, newHeight));
    setBottomPanelHeight(constrainedHeight);
    
    if (!isPanelOpen && newHeight > openThreshold) {
      // Just open the panel, height is already set above
      console.log('🟢 Opening panel from drag, height:', constrainedHeight);
      justOpenedPanel = true;
      setAssetPanelOpen(true);
    } else if (isPanelOpen && newHeight < snapToCloseThreshold && !justOpenedPanel) {
      // Close when dragged too low (but not if we just opened it)
      batch(() => {
        setAssetPanelOpen(false);
        setIsResizingBottom(false);
        setResizingPanels(false);
      });
      document.body.classList.remove('dragging-vertical');
    } else if (isPanelOpen && newHeight > snapToMaxThreshold) {
      // Snap to maximum height - right below the horizontal toolbar
      console.log('🟢 Snapping to maximum height - below horizontal toolbar');
      setBottomPanelHeight(maxPossibleHeight);
    }
  };

  const handleBottomResizeEnd = () => {
    console.log('🔴 Bottom panel resize END');
    setIsResizingBottom(false);
    setResizingPanels(false);
    document.body.classList.remove('dragging-vertical');
    justOpenedPanel = false; // Reset the flag
  };

  const handleRightResizeStart = () => {
    console.log('🟡 Right panel resize START');
    setIsResizingRight(true);
    setResizingPanels(true);
    document.body.classList.add('dragging-horizontal');
    
  };

  const handleRightResizeMove = (e, { isScenePanelOpen, isLeftPanel = false, selectedRightTool }) => {
    if (!isResizingRight()) return;
    
    const newWidth = isLeftPanel 
      ? e.clientX
      : window.innerWidth - e.clientX;
    
    const snapThreshold = 100;
    const openThreshold = 150;
    
    console.log('🟡 Right resize move:', { newWidth, isScenePanelOpen, clientX: e.clientX });
    
    if (!isScenePanelOpen && newWidth > openThreshold) {
      setScenePanelOpen(true);
      setRightPanelWidth(Math.max(304, newWidth));
      // Only set to 'scene' if current tool is 'select' or empty (same logic as handleRightPanelToggle)
      const currentTool = typeof selectedRightTool === 'function' ? selectedRightTool() : selectedRightTool;
      if (!currentTool || currentTool === 'select') {
        setSelectedRightTool('scene');
      }
    } else if (isScenePanelOpen && newWidth < snapThreshold) {
      setScenePanelOpen(false);
      setIsResizingRight(false);
      setResizingPanels(false);
      document.body.classList.remove('dragging-horizontal');
      // Don't change the selected tool when closing - keep the current tool
    } else if (isScenePanelOpen) {
      const constrainedWidth = Math.max(304, Math.min(600, newWidth));
      console.log('🟡 Setting right panel width to:', constrainedWidth);
      setRightPanelWidth(constrainedWidth);
    }
  };

  const handleRightResizeEnd = () => {
    console.log('🟠 Right panel resize END');
    setIsResizingRight(false);
    setResizingPanels(false);
    document.body.classList.remove('dragging-horizontal');
  };

  return {
    isResizingBottom,
    isResizingRight,
    handleBottomResizeStart,
    handleBottomResizeMove,
    handleBottomResizeEnd,
    handleRightResizeStart,
    handleRightResizeMove,
    handleRightResizeEnd
  };
};