import { useState, useEffect, useRef } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';

const ContextMenu = ({ items, position, onClose }) => {
  const menuRef = useRef(null);
  const submenuRef = useRef(null);
  const [menuPosition, setMenuPosition] = useState({ top: 0, left: 0 });
  const [hoveredItem, setHoveredItem] = useState(null);
  const [submenuPosition, setSubmenuPosition] = useState(null);
  const [submenuItems, setSubmenuItems] = useState(null);
  const hideSubmenuTimeout = useRef(null);

  useEffect(() => {
    const handleClickOutside = (event) => {
      const isInsideMenu = menuRef.current && menuRef.current.contains(event.target);
      const isInsideSubmenu = submenuRef.current && submenuRef.current.contains(event.target);
      
      if (!isInsideMenu && !isInsideSubmenu) {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [onClose]);

  useEffect(() => {
    if (menuRef.current) {
      const { innerWidth, innerHeight } = window;
      const { offsetWidth, offsetHeight } = menuRef.current;
      let { x, y } = position;

      if (x + offsetWidth > innerWidth) {
        x = innerWidth - offsetWidth;
      }

      if (y + offsetHeight > innerHeight) {
        y = innerHeight - offsetHeight;
      }

      setMenuPosition({ top: y, left: x });
    }
  }, [position]);

  const handleItemMouseEnter = (index, item, e) => {
    setHoveredItem(index);
    
    // Clear any pending hide timeout
    if (hideSubmenuTimeout.current) {
      clearTimeout(hideSubmenuTimeout.current);
      hideSubmenuTimeout.current = null;
    }
    
    if (item.submenu && item.submenu.length > 0) {
      const rect = e.currentTarget.getBoundingClientRect();
      const { innerWidth, innerHeight } = window;
      
      // Position submenu to the right by default
      let submenuX = rect.right + 2;
      let submenuY = rect.top;
      
      // Check if submenu would go off screen and adjust
      const estimatedSubmenuWidth = 180;
      if (submenuX + estimatedSubmenuWidth > innerWidth) {
        submenuX = rect.left - estimatedSubmenuWidth - 2;
      }
      
      // Ensure submenu doesn't go below viewport
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
    hideSubmenuTimeout.current = setTimeout(() => {
      setSubmenuPosition(null);
      setSubmenuItems(null);
    }, 300);
  };

  const handleItemMouseLeave = () => {
    // Keep submenu open when moving to submenu
    // Only clear if we're not hovering over an item with submenu
  };

  if (!items || items.length === 0) {
    return null;
  }

  return (
    <>
    <div
      ref={menuRef}
      className="bg-slate-800/95 backdrop-blur-md border border-slate-700 rounded-md shadow-xl py-1 pointer-events-auto"
      style={{ 
        position: 'fixed',
        top: menuPosition.top, 
        left: menuPosition.left,
        zIndex: 999998
      }}
      onMouseLeave={() => {
        setHoveredItem(null);
        hideSubmenu();
      }}
    >
      <ul>
        {items.map((item, index) => (
          <li key={index}>
            {item.separator ? (
              <div className="border-t border-slate-700 my-1" />
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
                className={`flex items-center w-full px-3 py-1.5 text-xs text-left transition-all duration-200 ${
                  hoveredItem === index 
                    ? 'bg-blue-600 text-white' 
                    : 'text-gray-300 hover:bg-blue-600/70 hover:text-white'
                }`}
              >
                {item.color && (
                  <div 
                    className="w-3 h-3 rounded-full mr-2 border border-gray-600" 
                    style={{ backgroundColor: item.color }}
                  />
                )}
                {item.icon && <span className="mr-2">{item.icon}</span>}
                <span className="flex-1">{item.label}</span>
                {item.submenu && <Icons.ChevronRight className="w-3 h-3 ml-1" />}
              </button>
            )}
          </li>
        ))}
      </ul>
      
    </div>
    
    {/* Submenu rendered separately to avoid nesting issues */}
    {submenuItems && submenuPosition && (
      <div
        ref={submenuRef}
        className="bg-slate-800/95 backdrop-blur-md border border-slate-700 rounded-md shadow-xl py-1 pointer-events-auto min-w-[180px]"
        style={{ 
          position: 'fixed',
          top: submenuPosition.top, 
          left: submenuPosition.left,
          zIndex: 999999
        }}
        onMouseEnter={() => {
          // Clear hide timeout when entering submenu
          if (hideSubmenuTimeout.current) {
            clearTimeout(hideSubmenuTimeout.current);
            hideSubmenuTimeout.current = null;
          }
        }}
        onMouseLeave={() => {
          hideSubmenu();
        }}
      >
        <ul>
          {submenuItems.map((subItem, subIndex) => (
            <li key={subIndex}>
              {subItem.separator ? (
                <div className="border-t border-slate-700 my-1" />
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
                  className="flex items-center w-full px-3 py-1.5 text-xs text-left text-gray-300 transition-all duration-200 hover:bg-blue-600/70 hover:text-white"
                >
                  {subItem.color && (
                    <div 
                      className="w-3 h-3 rounded-full mr-2 border border-gray-600" 
                      style={{ backgroundColor: subItem.color }}
                    />
                  )}
                  {subItem.icon && <span className="mr-2">{subItem.icon}</span>}
                  <span className="flex-1">{subItem.label}</span>
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
