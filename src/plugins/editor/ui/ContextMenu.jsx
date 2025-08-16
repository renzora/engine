import { createSignal, createEffect, onCleanup } from 'solid-js';
import { IconChevronRight } from '@tabler/icons-solidjs';

const ContextMenu = ({ items, position, onClose }) => {
  let menuRef;
  let submenuRef;
  const [menuPosition, setMenuPosition] = createSignal({ top: 0, left: 0 });
  const [hoveredItem, setHoveredItem] = createSignal(null);
  const [submenuPosition, setSubmenuPosition] = createSignal(null);
  const [submenuItems, setSubmenuItems] = createSignal(null);
  let hideSubmenuTimeout = null;

  createEffect(() => {
    const handleClickOutside = (event) => {
      const isInsideMenu = menuRef && menuRef.contains(event.target);
      const isInsideSubmenu = submenuRef && submenuRef.contains(event.target);
      
      if (!isInsideMenu && !isInsideSubmenu) {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    onCleanup(() => {
      document.removeEventListener('mousedown', handleClickOutside);
    });
  });

  createEffect(() => {
    if (menuRef) {
      const { innerWidth, innerHeight } = window;
      const { offsetWidth, offsetHeight } = menuRef;
      let { x, y } = position;

      if (x + offsetWidth > innerWidth) {
        x = innerWidth - offsetWidth;
      }

      if (y + offsetHeight > innerHeight) {
        y = innerHeight - offsetHeight;
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
      let submenuX = rect.right + 2;
      let submenuY = rect.top;
      const estimatedSubmenuWidth = 180;
      if (submenuX + estimatedSubmenuWidth > innerWidth) {
        submenuX = rect.left - estimatedSubmenuWidth - 2;
      }
      
      const estimatedSubmenuHeight = item.submenu.length * 32;
      if (submenuY + estimatedSubmenuHeight > innerHeight) {
        submenuY = innerHeight - estimatedSubmenuHeight - 10;
      }
      
      setSubmenuPosition({ top: submenuY, left: submenuX });
      setSubmenuItems(item.submenu);
      
    } else {
      setSubmenuPosition(null);
      setSubmenuItems(null);
    }
  };

  const hideSubmenu = () => {
    hideSubmenuTimeout = setTimeout(() => {
      setSubmenuPosition(null);
      setSubmenuItems(null);
    }, 300);
  };

  const handleItemMouseLeave = () => {
    // Empty for now
  };

  onCleanup(() => {
    if (hideSubmenuTimeout) {
      clearTimeout(hideSubmenuTimeout);
    }
  });

  if (!items || items.length === 0) {
    return null;
  }

  return (
    <>
      <div
        ref={menuRef}
        class="bg-slate-800/95 backdrop-blur-md border border-slate-700 rounded-md shadow-xl py-1 pointer-events-auto"
        style={{ 
          position: 'fixed',
          top: `${menuPosition().top}px`, 
          left: `${menuPosition().left}px`,
          'z-index': '999998'
        }}
        onMouseLeave={() => {
          setHoveredItem(null);
          hideSubmenu();
        }}
      >
        <ul>
          {items.map((item, index) => (
            <li>
              {item.separator ? (
                <div class="border-t border-slate-700 my-1" />
              ) : (
                <button
                  onMouseEnter={(e) => handleItemMouseEnter(index, item, e)}
                  onMouseLeave={() => handleItemMouseLeave()}
                  onClick={() => {
                    if (!item.submenu) {
                      item.action();
                      onClose();
                    }
                  }}
                  class={`flex items-center w-full px-3 py-1.5 text-xs text-left transition-all duration-200 ${
                    hoveredItem() === index 
                      ? 'bg-blue-600 text-white' 
                      : 'text-gray-300 hover:bg-blue-600/70 hover:text-white'
                  }`}
                >
                  {item.color && (
                    <div 
                      class="w-3 h-3 rounded-full mr-2 border border-gray-600" 
                      style={{ 'background-color': item.color }}
                    />
                  )}
                  {item.icon && <span class="mr-2">{item.icon}</span>}
                  <span class="flex-1">{item.label}</span>
                  {item.submenu && <IconChevronRight class="w-3 h-3 ml-1" />}
                </button>
              )}
            </li>
          ))}
        </ul>
        
      </div>
      
      {submenuItems() && submenuPosition() && (
        <div
          ref={submenuRef}
          class="bg-slate-800/95 backdrop-blur-md border border-slate-700 rounded-md shadow-xl py-1 pointer-events-auto min-w-[180px]"
          style={{ 
            position: 'fixed',
            top: `${submenuPosition().top}px`, 
            left: `${submenuPosition().left}px`,
            'z-index': '999999'
          }}
          onMouseEnter={() => {
            if (hideSubmenuTimeout) {
              clearTimeout(hideSubmenuTimeout);
              hideSubmenuTimeout = null;
            }
          }}
          onMouseLeave={() => {
            hideSubmenu();
          }}
        >
          <ul>
            {submenuItems().map((subItem, subIndex) => (
              <li>
                {subItem.separator ? (
                  <div class="border-t border-slate-700 my-1" />
                ) : (
                  <button
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      try {
                        subItem.action();
                        onClose();
                      } catch (error) {
                        console.error('Error executing submenu action:', error);
                        onClose();
                      }
                    }}
                    class="flex items-center w-full px-3 py-1.5 text-xs text-left text-gray-300 transition-all duration-200 hover:bg-blue-600/70 hover:text-white"
                  >
                    {subItem.color && (
                      <div 
                        class="w-3 h-3 rounded-full mr-2 border border-gray-600" 
                        style={{ 'background-color': subItem.color }}
                      />
                    )}
                    {subItem.icon && <span class="mr-2">{subItem.icon}</span>}
                    <span class="flex-1">{subItem.label}</span>
                  </button>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
    </>
  );
};

export default ContextMenu;