import { createSignal, createEffect, onCleanup, For, Show } from 'solid-js';
import { IconChevronRight } from '@tabler/icons-solidjs';

// Recursive submenu component for unlimited nesting
const SubmenuRenderer = ({ items, level = 0, onClose, position, preferredDirection = null }) => {
  let menuRef;
  const [menuPosition, setMenuPosition] = createSignal(position || { top: 0, left: 0 });
  const [hoveredItem, setHoveredItem] = createSignal(null);
  const [activeSubmenu, setActiveSubmenu] = createSignal(null);
  const [submenuDirection, setSubmenuDirection] = createSignal(preferredDirection);
  let hideSubmenuTimeout = null;

  // Position the menu within viewport bounds
  createEffect(() => {
    if (menuRef && position) {
      const { innerWidth, innerHeight } = window;
      const { offsetWidth, offsetHeight } = menuRef;
      let { x, y } = position;

      if (x + offsetWidth > innerWidth) {
        x = innerWidth - offsetWidth - 10;
      }

      if (y + offsetHeight > innerHeight) {
        y = innerHeight - offsetHeight - 10;
      }

      setMenuPosition({ top: y, left: x });
    }
  });

  const handleItemMouseEnter = (index, item, e) => {
    setHoveredItem(index);
    
    if (hideSubmenuTimeout) {
      clearTimeout(hideSubmenuTimeout);
      hideSubmenuTimeout = null;
    }
    
    if (item.submenu && item.submenu.length > 0) {
      const rect = e.currentTarget.getBoundingClientRect();
      const { innerWidth, innerHeight } = window;
      const estimatedSubmenuWidth = 180;
      let submenuX, submenuY = rect.top;
      let currentDirection = submenuDirection();
      
      // Determine positioning direction
      if (currentDirection === null) {
        // First level - check boundary and set direction preference
        const rightSpace = innerWidth - rect.right;
        const leftSpace = rect.left;
        
        if (rightSpace >= estimatedSubmenuWidth) {
          // Enough space on right, go right
          submenuX = rect.right + 2;
          currentDirection = 'right';
        } else if (leftSpace >= estimatedSubmenuWidth) {
          // Not enough space on right, go left
          submenuX = rect.left - estimatedSubmenuWidth - 2;
          currentDirection = 'left';
        } else {
          // Very limited space, default to right with viewport clipping
          submenuX = rect.right + 2;
          currentDirection = 'right';
        }
        
        // Set direction for all subsequent levels
        setSubmenuDirection(currentDirection);
      } else {
        // Subsequent levels - follow the established direction
        if (currentDirection === 'right') {
          submenuX = rect.right + 2;
          // Ensure we don't go off screen
          if (submenuX + estimatedSubmenuWidth > innerWidth) {
            submenuX = innerWidth - estimatedSubmenuWidth - 10;
          }
        } else {
          submenuX = rect.left - estimatedSubmenuWidth - 2;
          // Ensure we don't go off screen
          if (submenuX < 0) {
            submenuX = 10;
          }
        }
      }
      
      // Vertical positioning
      const estimatedSubmenuHeight = item.submenu.length * 32;
      if (submenuY + estimatedSubmenuHeight > innerHeight) {
        submenuY = innerHeight - estimatedSubmenuHeight - 10;
      }
      
      setActiveSubmenu({
        items: item.submenu,
        position: { x: submenuX, y: submenuY },
        level: level + 1,
        direction: currentDirection
      });
    } else {
      setActiveSubmenu(null);
    }
  };

  const handleItemMouseLeave = () => {
    hideSubmenuTimeout = setTimeout(() => {
      setActiveSubmenu(null);
    }, 100);
  };

  const handleSubmenuMouseEnter = () => {
    if (hideSubmenuTimeout) {
      clearTimeout(hideSubmenuTimeout);
      hideSubmenuTimeout = null;
    }
  };

  const handleSubmenuMouseLeave = () => {
    hideSubmenuTimeout = setTimeout(() => {
      setActiveSubmenu(null);
    }, 100);
  };

  return (
    <>
      <div
        ref={menuRef}
        data-context-menu
        class="bg-base-200/95 backdrop-blur-md border border-base-300 rounded-md shadow-xl py-1 pointer-events-auto min-w-[180px]"
        style={{ 
          position: 'fixed',
          top: `${menuPosition().top}px`, 
          left: `${menuPosition().left}px`,
          'z-index': `${999998 + level}`
        }}
        onMouseLeave={() => {
          setHoveredItem(null);
          handleItemMouseLeave();
        }}
      >
        <ul>
          <For each={items}>
            {(item, index) => (
              <li>
                <Show when={item.separator} fallback={
                  <button
                    onMouseEnter={(e) => handleItemMouseEnter(index(), item, e)}
                    onClick={() => {
                      if (!item.submenu) {
                        try {
                          item.action?.();
                          onClose();
                        } catch (error) {
                          console.error('Error executing menu action:', error);
                          onClose();
                        }
                      }
                    }}
                    class={`flex items-center w-full px-3 py-1.5 text-xs text-left transition-all duration-200 ${
                      hoveredItem() === index() 
                        ? 'bg-primary text-primary-content' 
                        : 'text-base-content/70 hover:bg-primary/70 hover:text-primary-content'
                    }`}
                  >
                    <Show when={item.color}>
                      <div 
                        class="w-3 h-3 rounded-full mr-2 border border-base-300" 
                        style={{ 'background-color': item.color }}
                      />
                    </Show>
                    <Show when={item.icon}>
                      <span class="mr-2">{item.icon}</span>
                    </Show>
                    <span class="flex-1">{item.label}</span>
                    <Show when={item.submenu}>
                      <IconChevronRight class="w-3 h-3 ml-1" />
                    </Show>
                  </button>
                }>
                  <div class="border-t border-base-300 my-1" />
                </Show>
              </li>
            )}
          </For>
        </ul>
      </div>
      
      <Show when={activeSubmenu()}>
        <div
          onMouseEnter={handleSubmenuMouseEnter}
          onMouseLeave={handleSubmenuMouseLeave}
        >
          <SubmenuRenderer
            items={activeSubmenu().items}
            level={activeSubmenu().level}
            position={activeSubmenu().position}
            preferredDirection={activeSubmenu().direction}
            onClose={onClose}
          />
        </div>
      </Show>
    </>
  );
};

const ContextMenu = ({ items, position, onClose }) => {
  createEffect(() => {
    const handleClickOutside = (event) => {
      // Check if click is inside any context menu at any level
      const contextMenus = document.querySelectorAll('[data-context-menu]');
      const isInsideAnyMenu = Array.from(contextMenus).some(menu => menu.contains(event.target));
      
      if (!isInsideAnyMenu) {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    onCleanup(() => {
      document.removeEventListener('mousedown', handleClickOutside);
    });
  });

  return (
    <SubmenuRenderer 
      items={items} 
      level={0} 
      position={position} 
      onClose={onClose} 
    />
  );
};

export default ContextMenu;