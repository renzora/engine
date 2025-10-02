import { createSignal, createEffect, onCleanup, For, Show } from 'solid-js';
import { horizontalMenuButtonsEnabled } from '@/api/plugin';
import { toolbarButtons } from '@/api/plugin';

function Helper(props) {
  const [activePluginDropdown, setActivePluginDropdown] = createSignal(null);
  const [pluginDropdownPosition, setPluginDropdownPosition] = createSignal(null);
  const [currentCameraView, setCurrentCameraView] = createSignal("Camera");

  // Watch for changes to global camera view name
  createEffect(() => {
    const checkCameraView = () => {
      if (window._currentCameraViewName && window._currentCameraViewName !== currentCameraView()) {
        setCurrentCameraView(window._currentCameraViewName);
      }
    };
    
    // Check immediately and then periodically
    checkCameraView();
    const interval = setInterval(checkCameraView, 100);
    
    onCleanup(() => clearInterval(interval));
  });
  
  // Expose close function globally so camera dropdown can call it
  window._closeHelperDropdowns = () => {
    setActivePluginDropdown(null);
    setPluginDropdownPosition(null);
  };
  
  createEffect(() => {
    const handleClickOutside = (event) => {
      const target = event.target;
      const isHelperButton = target.closest('.helper-button');
      const isDropdownContent = target.closest('.dropdown-content');
      
      if (!isHelperButton && !isDropdownContent) {
        setActivePluginDropdown(null);
        setPluginDropdownPosition(null);
      }
    };

    document.addEventListener('click', handleClickOutside);
    
    onCleanup(() => {
      document.removeEventListener('click', handleClickOutside);
    });
  });

  const getDropdownPosition = (buttonElement) => {
    if (!buttonElement) return null;
    
    const buttonRect = buttonElement.getBoundingClientRect();
    
    // Get the active button to determine dropdown width
    const activeButton = Array.from(toolbarButtons().values())
      .find(b => b.id === activePluginDropdown());
    const dropdownWidth = activeButton?.dropdownWidth || 280;
    
    // Center the dropdown horizontally relative to the button
    const buttonCenterX = buttonRect.left + (buttonRect.width / 2);
    const left = buttonCenterX - (dropdownWidth / 2);
    
    // Ensure dropdown doesn't go off-screen
    const minLeft = 8; // 8px margin from left edge
    const maxLeft = window.innerWidth - dropdownWidth - 8; // 8px margin from right edge
    const clampedLeft = Math.max(minLeft, Math.min(left, maxLeft));
    
    return {
      left: clampedLeft,
      top: buttonRect.bottom + 4
    };
  };

  return (
    <div class="flex items-center gap-1 pr-2">
      <For each={Array.from(toolbarButtons().values()).filter(button => button.section === 'helper').sort((a, b) => (a.order || 0) - (b.order || 0))}>
        {(button) => {
          const isEnabled = () => horizontalMenuButtonsEnabled();

          // Handle custom component buttons
          if (button.isCustomComponent && button.customComponent) {
            const CustomComponent = button.customComponent;
            return (
              <div class="flex items-center" title={button.title}>
                <CustomComponent />
              </div>
            );
          }

          return (
            <button
                class={`helper-button px-2 py-1 rounded transition-all duration-200 group relative ${
                  isEnabled() 
                    ? 'text-base-content/60 hover:text-base-content hover:bg-base-100/80 active:bg-base-200/80' 
                    : 'text-base-content/20 cursor-not-allowed'
                } ${
                  button.hasDropdown && activePluginDropdown() === button.id 
                    ? 'bg-base-200/80 text-base-content' 
                    : ''
                }`}
                onClick={(e) => {
                  if (!isEnabled()) return;
                  
                  // Close camera dropdown when any helper is clicked
                  if (props.onHelperClick) {
                    props.onHelperClick();
                  }
                  
                  if (button.hasDropdown) {
                    e.stopPropagation();
                    
                    if (activePluginDropdown() === button.id) {
                      setActivePluginDropdown(null);
                      setPluginDropdownPosition(null);
                    } else {
                      const position = getDropdownPosition(e.currentTarget);
                      setActivePluginDropdown(button.id);
                      setPluginDropdownPosition(position);
                    }
                  } else if (button.onClick) {
                    button.onClick();
                  }
                }}
                disabled={!isEnabled()}
                title={button.title}
              >
                <Show when={button.icon}>
                  <button.icon class="w-4 h-4" />
                </Show>
                <Show when={button.id === 'camera' && button.dynamicLabel}>
                  <span class="text-xs ml-1">
                    {currentCameraView()}
                  </span>
                </Show>
              </button>
          );
        }}
      </For>

      {/* Plugin dropdowns */}
      {activePluginDropdown() && pluginDropdownPosition() && (
        <div 
          class="dropdown-content fixed bg-base-200 backdrop-blur-sm rounded-lg shadow-xl border border-base-300 z-[210] text-base-content text-xs"
          style={{
            left: `${pluginDropdownPosition().left}px`,
            top: `${pluginDropdownPosition().top}px`
          }}
        >
          {(() => {
            const activeButton = Array.from(toolbarButtons().values())
              .find(b => b.id === activePluginDropdown());
            if (activeButton && activeButton.dropdownComponent) {
              const DropdownComponent = activeButton.dropdownComponent;
              return <DropdownComponent />;
            }
            return null;
          })()}
        </div>
      )}
    </div>
  );
}

export default Helper;