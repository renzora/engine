ui = {
  notificationCount: 0,
  activeNotifications: new Map(),
  tabs: {},
  menus: {},
  accordions: {},
  activeMenuId: null,
  activeSubItemIndex: 0,
  toggleSwitches: {},

  notif: function(id, message, replace = false) {
      return new Promise(resolve => {
        audio.playAudio("notification", assets.use('notification'), 'sfx', false);
          let container = document.getElementById('notification');
          if (!container) {
              container = document.createElement('div');
              container.id = 'notification';
              container.className = 'fixed z-10 top-0 left-1/2 transform -translate-x-1/2';
              document.body.appendChild(container);
          }

          if (this.activeNotifications.has(id)) {
              const existingNotification = this.activeNotifications.get(id);
              if (replace) {
                  existingNotification.innerText = message;

                  // Clear the existing timer and reset it
                  clearTimeout(existingNotification.timer);
                  existingNotification.timer = setTimeout(() => {
                      existingNotification.classList.add('notification-exit');
                      setTimeout(() => {
                          existingNotification.remove();
                          this.notificationCount--;
                          this.activeNotifications.delete(id);

                          if (this.notificationCount === 0) {
                              container.remove();
                          }
                          resolve();
                      }, 1000);
                  }, 3000);
                  return;
              } else {
                  resolve();
                  return;
              }
          }

          const notification = document.createElement('div');
          notification.className = 'notif text-white text-lg px-4 py-2 rounded shadow-md m-2';
          notification.innerText = message; // Use the message parameter
          notification.dataset.id = id; // Assign id to notification element
          container.prepend(notification);

          this.notificationCount++;
          this.activeNotifications.set(id, notification); // Add id to active notifications map

          // Set and store the timer
          notification.timer = setTimeout(() => {
              notification.classList.add('notification-exit');

              setTimeout(() => {
                  notification.remove();
                  this.notificationCount--;
                  this.activeNotifications.delete(id); // Remove id from active notifications map

                  if (this.notificationCount === 0) {
                      container.remove();
                  }
                  resolve();
              }, 1000);
          }, 3000);
      });
  },
  html: function(selectorOrElement, htmlString, action = 'replace') {
    const element = (typeof selectorOrElement === 'string') ? document.querySelector(selectorOrElement) : selectorOrElement;

    if (!element) {
        return;
    }

    switch (action) {
        case 'append':
            element.insertAdjacentHTML('beforeend', htmlString);
            break;
        case 'prepend':
            element.insertAdjacentHTML('afterbegin', htmlString);
            break;
        case 'html':
        default:
            element.innerHTML = htmlString;
            break;
    }

    // Execute scripts from the HTML string
    const tempContainer = document.createElement('div');
    tempContainer.innerHTML = htmlString;
    Array.from(tempContainer.querySelectorAll('script')).forEach(oldScript => {
        const newScript = document.createElement('script');
        if (oldScript.src) {
            // If the script tag has a src attribute, set it on the new script element
            newScript.src = oldScript.src;
            newScript.async = false; // Ensure scripts are executed in order
        } else {
            // If it's an inline script, set its text content
            newScript.textContent = oldScript.textContent;
        }
        // Copy over any other attributes
        Array.from(oldScript.attributes).forEach(attr => newScript.setAttribute(attr.name, attr.value));
        document.body.appendChild(newScript);
        // Remove the script tag after it is executed
        document.body.removeChild(newScript);
    });
},

unmount: function(id) {
    console.log("attempting to unmount", id);
    
    if (window[id] && typeof window[id].unmount === 'function') {
        window[id].unmount();  // Call the object's unmount function if it exists
    }

    var obj = window[id];

    if (obj) {
        // Clear any arrays or object references (e.g., event listeners)
        if (obj.eventListeners && Array.isArray(obj.eventListeners)) {
            obj.eventListeners.length = 0;  // Clear event listeners array
        }

        // Clear all properties of the object
        for (var prop in obj) {
            if (obj.hasOwnProperty(prop)) {
                if (typeof obj[prop] === "function") {
                    delete obj[prop];  // Delete functions
                } else if (Array.isArray(obj[prop])) {
                    obj[prop] = [];  // Clear arrays
                } else if (typeof obj[prop] === "object" && obj[prop] !== null) {
                    obj[prop] = {};  // Clear nested objects
                } else {
                    obj[prop] = null;  // Clear primitive types
                }
            }
        }

        // Delete the object reference from the window
        delete window[id];
        console.log(id, "has been completely unmounted and deleted.");
    }
},

ajax: async function({ url, method = 'GET', data = null, outputType = 'text', success, error }) {
    try {
      let fetchUrl = url;
      const init = {
        method: method,
        headers: {}
      };
  
      if (data) {
        if (method === 'GET') {
          const queryParams = new URLSearchParams(data).toString();
          fetchUrl = `${url}?${queryParams}`;
        } else {
          if (typeof data === 'object') {
            // Assuming data is an object, stringify it for JSON
            init.headers['Content-Type'] = 'application/json';
            init.body = JSON.stringify(data);
          } else {
            // If data is already a string, assume it's URL-encoded
            init.headers['Content-Type'] = 'application/x-www-form-urlencoded';
            init.body = data;
          }
        }
      }
  
      const response = await fetch(fetchUrl, init);
  
      if (!response.ok) {
        // Handle response errors
        const errorText = await response.text(); // Get the response text for debugging
        throw new Error(errorText);
      }
  
      let responseData;
      switch (outputType) {
        case 'json':
          responseData = await response.json();
          break;
        case 'blob':
          responseData = await response.blob();
          break;
        case 'formData':
          responseData = await response.formData();
          break;
        case 'arrayBuffer':
          responseData = await response.arrayBuffer();
          break;
        default:
          responseData = await response.text();
      }
  
      if (success) success(responseData);
  
    } catch (err) {
      console.error('Failed to save data:', err);
      if (error) {
        // Check if the error is a string (from the fetch error handling) or a standard Error object
        if (err instanceof Error) {
          error(err.message); // Pass the error message to the callback
        } else {
          error(err); // Pass the generic error object
        }
      }
    }
  },

    initTabs: function(containerId, defaultTab) {
        const container = document.getElementById(containerId);
        if (!container) return;

        const tabButtons = container.querySelectorAll('[data-tab]');
        const tabContents = container.querySelectorAll('[data-tab-content]');

        tabButtons.forEach(button => {
            button.setAttribute('data-container', containerId);
            button.addEventListener('click', () => {
                const target = button.getAttribute('data-tab');
                const containerId = button.getAttribute('data-container');
                this.showTab(target, containerId);
                audio.playAudio("click", assets.use('click'), 'sfx');
            });
        });

        tabContents.forEach(content => {
            content.setAttribute('data-container', containerId);
        });

        // Store the initialized tabs and contents
        this.tabs[containerId] = { tabButtons, tabContents };

        // Set the default active tab
        const initialTab = defaultTab || tabButtons[0].getAttribute('data-tab');
        this.showTab(initialTab, containerId);
    },

    showTab: function(target, containerId) {
      const { tabButtons, tabContents } = this.tabs[containerId];
  
      tabButtons.forEach(button => {
          if (button.getAttribute('data-container') === containerId) {
              button.classList.remove('active');
              if (button.getAttribute('data-tab') === target) {
                  button.classList.add('active');
              }
          }
      });
  
      tabContents.forEach(content => {
          if (content.getAttribute('data-container') === containerId) {
              content.classList.remove('active');
              if (content.getAttribute('data-tab-content') === target) {
                  content.classList.add('active');
  
                  // Check for data-menu attribute
                  const tabButton = content.previousElementSibling.querySelector(`[data-tab="${target}"]`);
                  const menuId = tabButton && tabButton.getAttribute('data-menu');
                  if (menuId && this.menus[menuId]) {
                      this.setActiveMenu(menuId);
                  }
              }
          }
      });
  },
  

    switchTab: function(containerId, direction) {
      const { tabButtons } = this.tabs[containerId];
      if (!tabButtons) return;

      let activeIndex = Array.from(tabButtons).findIndex(tab => tab.classList.contains('active'));
      if (activeIndex === -1) return;

      const newIndex = (activeIndex + direction + tabButtons.length) % tabButtons.length;
      const targetTab = tabButtons[newIndex].getAttribute('data-tab');
      this.showTab(targetTab, containerId);
  },

    destroyTabs: function(containerId) {
        const container = document.getElementById(containerId);
        if (!container || !this.tabs[containerId]) return;

        const { tabButtons, tabContents } = this.tabs[containerId];

        // Remove event listeners
        tabButtons.forEach(button => {
            button.replaceWith(button.cloneNode(true));
        });

        // Clear the tabButtons and tabContents
        tabButtons.forEach(button => button.remove());
        tabContents.forEach(content => content.remove());

        // Remove the stored reference
        delete this.tabs[containerId];
    },

    initMenu: function(containerId, defaultMenuItem) {
      const container = document.getElementById(containerId);
      if (!container) return;
  
      const menuItems = Array.from(container.querySelectorAll('[data-menu-item]'));
  
      menuItems.forEach(item => {
          item.setAttribute('data-container', containerId);
      });
  
      // Store the initialized menu items
      this.menus[containerId] = { menuItems };
  
      // Set the default active menu item
      const initialMenuItem = defaultMenuItem || menuItems[0]?.getAttribute('data-menu-item');
      if (initialMenuItem) {
          this.showMenuItem(initialMenuItem, containerId);
      }
  
      // Set the active menu ID
      this.activeMenuId = containerId;
  },

    showMenuItem: function(target, containerId) {
      const { menuItems } = this.menus[containerId];
    
      menuItems.forEach(item => {
        if (item.getAttribute('data-container') === containerId) {
          if (item.getAttribute('data-menu-item') === target) {
            item.classList.add('bg-blue-600', 'text-white', 'active');  // Add active state
            item.classList.remove('text-gray-300');
          } else {
            item.classList.remove('bg-blue-600', 'text-white', 'active');  // Remove active state
            item.classList.add('text-gray-300');
          }
        }
      });
    },

    setActiveMenu: function(containerId) {
      // Deactivate all menus
      for (const id in this.menus) {
        const { menuItems } = this.menus[id];
        menuItems.forEach(item => {
          item.classList.add('text-gray-300');
          item.classList.remove('bg-blue-600', 'text-white', 'active');
        });
      }
    
      // Activate the specified menu
      const { menuItems } = this.menus[containerId];
      menuItems.forEach((item, index) => {
        if (index === 0) {
          item.classList.add('bg-blue-600', 'text-white', 'active');  // Set first item active
          item.classList.remove('text-gray-300');
        } else {
          item.classList.add('text-gray-300');
          item.classList.remove('bg-blue-600', 'text-white', 'active');
        }
      });
    
      // Set the activeMenuId to the new active menu
      this.activeMenuId = containerId;
    },

    highlightMenuItem: function(containerId, direction) {
      const { menuItems } = this.menus[containerId];
      const activeIndex = Array.from(menuItems).findIndex(item => item.classList.contains('active'));

      let newIndex = (activeIndex + direction + menuItems.length) % menuItems.length;

      // Remove active classes from current item
      if (activeIndex !== -1) {
          menuItems[activeIndex].classList.remove('active', 'bg-blue-600', 'text-white');
          menuItems[activeIndex].classList.add('text-gray-300');
      }

      // Add active classes to the new item
      const newItem = menuItems[newIndex];
      newItem.classList.add('active', 'bg-blue-600', 'text-white');
      newItem.classList.remove('text-gray-300');

      this.activeSubItemIndex = 0; // Reset sub-item index when switching menu items
  },
  focusSubItem: function(containerId, direction) {
    const menuItems = this.menus[containerId].menuItems;
    const activeItem = menuItems.find(item => item.classList.contains('active'));
    if (!activeItem) return;

    const controls = activeItem.querySelectorAll('[data-control]');
    if (controls.length === 0) return;

    this.activeSubItemIndex = (this.activeSubItemIndex + direction + controls.length) % controls.length;
    controls.forEach((control, index) => {
        control.classList.toggle('focused', index === this.activeSubItemIndex);
    });

    controls[this.activeSubItemIndex].focus();
},

interactWithHighlightedItem: function(containerId) {
  const { menuItems } = this.menus[containerId];
  const activeItem = menuItems.find(item => item.classList.contains('active'));

  if (!activeItem) {
      console.warn(`No active item found in container '${containerId}'.`);
      return;
  }

  console.log(`Active item found:`, activeItem);

  // Attempt to find the control by its data-control attribute
  let control = activeItem.querySelector('[data-control]');
  if (!control) {
      console.warn(`No control found within the active item.`);
      return;
  }

  console.log(`Control found within active item:`, control);

  if (control.classList.contains('custom-dropdown-trigger')) {
      // If the control is the custom dropdown trigger, toggle the dropdown
      this.toggleCustomDropdown(control, control.nextElementSibling); // The dropdown menu is the next sibling

      // Prevent further navigation if the dropdown is open
      if (!control.nextElementSibling.classList.contains('hidden')) {
          this.dropdownOpen = true;
      } else {
          this.dropdownOpen = false;
      }
  } else if (control.tagName === 'INPUT') {
      if (control.type === 'checkbox') {
          // Toggle checkbox
          control.checked = !control.checked;
          console.log(`Checkbox toggled. New state: ${control.checked}`);
      } else if (control.type === 'range') {
          // Handle range input (if needed)
          console.log(`Range control found. Current value: ${control.value}`);
      }
  } else if (control.classList.contains('toggle')) {
      // Handle custom toggle switch
      const toggleSwitch = this.toggleSwitches[control.id];
      if (toggleSwitch) {
          toggleSwitch.toggle();
          console.log(`Toggled switch '${control.id}'. New state: ${toggleSwitch.state()}`);
      } else {
          console.error(`No toggle switch found with id '${control.id}' in ui.toggleSwitches.`);
      }
  } else {
      console.warn(`Control type not recognized or unsupported.`);
  }
},


adjustSlider: function(containerId, direction) {
  const menuData = this.menus[containerId];
  if (!menuData || !Array.isArray(menuData.menuItems)) {
      console.error('No valid menu items found for containerId:', containerId);
      return;
  }

  const menuItems = menuData.menuItems;
  if (menuItems.length === 0) return;

  const activeItem = menuItems.find(item => item.classList.contains('active'));
  if (!activeItem) {
      console.error('No active menu item found.');
      return;
  }

  const control = activeItem.querySelector('[data-control]');

  if (control && control.tagName === 'INPUT' && control.type === 'range') {
      const stepMultiplier = 5;  // Increase this value to make the slider move faster
      const step = direction * ((parseInt(control.step) || 1) * stepMultiplier);
      const newValue = parseInt(control.value) + step;

      if (newValue >= control.min && newValue <= control.max) {
          control.value = newValue;
      } else if (newValue < control.min) {
          control.value = control.min;
      } else if (newValue > control.max) {
          control.value = control.max;
      }
  } else {
      console.error('No valid range control found in active item.');
  }
},

  destroyMenu: function(containerId) {
      const container = document.getElementById(containerId);
      if (!container || !this.menus[containerId]) return;

      const { menuItems } = this.menus[containerId];

      // Clear the menuItems
      menuItems.forEach(item => item.remove());

      // Remove the stored reference
      delete this.menus[containerId];
  },

  createToggleSwitch: function(containerId, switchId, initialState = false) {
    console.log(`Attempting to create toggle switch in container: ${containerId} with id: ${switchId}`);
    
    const container = document.getElementById(containerId);
    if (!container) {
        console.error(`Container with id '${containerId}' not found.`);
        return;
    }

    console.log(`Container found. Creating toggle switch.`);

    const switchButton = document.createElement('button');
    switchButton.id = switchId;
    switchButton.setAttribute('data-control', switchId); // Adding data-control attribute here
    switchButton.className = `toggle relative w-12 h-6 rounded-full focus:outline-none transition-colors ${
        initialState ? 'bg-green-500' : 'bg-gray-400'
    }`;

    const switchCircle = document.createElement('span');
    switchCircle.className = `toggle-circle absolute top-0 left-0 w-6 h-6 rounded-full shadow transition transform ${
        initialState ? 'translate-x-full bg-white' : 'translate-x-0 bg-white'
    }`;

    switchButton.appendChild(switchCircle);
    container.appendChild(switchButton);

    console.log(`Toggle switch with id '${switchId}' created and added to container '${containerId}'. Initial state: ${initialState}`);

    function toggleSwitch() {
        const isOn = switchButton.classList.toggle('bg-green-500');
        switchButton.classList.toggle('bg-gray-400', !isOn);
        switchCircle.classList.toggle('translate-x-full', isOn);
        switchCircle.classList.toggle('translate-x-0', !isOn);
        console.log(`Toggle switch '${switchId}' state changed to: ${isOn}`);
    }

    switchButton.addEventListener('click', toggleSwitch);

    ui.toggleSwitches[switchId] = {
        element: switchButton,
        toggle: toggleSwitch,
        state: () => switchButton.classList.contains('bg-green-500'),
    };

    console.log(`Toggle switch '${switchId}' is now stored in ui.toggleSwitches.`);

    return ui.toggleSwitches[switchId];
},

createCustomDropdown: function(containerId, dropdownId, options = []) {
  console.log(`Attempting to create custom dropdown in container: ${containerId} with id: ${dropdownId}`);
  
  const container = document.getElementById(containerId);
  if (!container) {
      console.error(`Container with id '${containerId}' not found.`);
      return;
  }

  // Find the menu item element by its data-menu-item attribute
  const menuItem = container.querySelector('[data-menu-item="item3"]');
  if (!menuItem) {
      console.error(`Menu item with data-menu-item="item3" not found.`);
      return;
  }

  console.log(`Container and menu item found. Creating custom dropdown.`);

  // Create the dropdown trigger element
  const dropdownTrigger = document.createElement('div');
  dropdownTrigger.id = dropdownId;
  dropdownTrigger.className = 'custom-dropdown-trigger cursor-pointer bg-gray-800 text-white rounded-md p-2 mt-2';
  dropdownTrigger.innerText = 'Select Option';
  dropdownTrigger.setAttribute('data-control', dropdownId);

  // Create the dropdown menu container
  const dropdownMenu = document.createElement('div');
  dropdownMenu.className = 'custom-dropdown-menu hidden bg-gray-700 rounded-md mt-2 shadow-lg';

  // Populate the dropdown menu with options
  options.forEach((optionText, index) => {
      const option = document.createElement('div');
      option.className = 'dropdown-option p-2 text-gray-300 cursor-pointer hover:bg-gray-600';
      option.innerText = optionText;
      option.setAttribute('data-value', optionText);
      option.dataset.index = index;
      dropdownMenu.appendChild(option);
  });

  // Append the trigger and menu to the menu item
  const dropdownContainer = document.createElement('div');
  dropdownContainer.className = 'custom-dropdown mt-2';
  dropdownContainer.appendChild(dropdownTrigger);
  dropdownContainer.appendChild(dropdownMenu);

  menuItem.appendChild(dropdownContainer);

  console.log(`Custom dropdown with id '${dropdownId}' created and added to menu item '${menuItem.getAttribute('data-menu-item')}'.`);

  // Store the dropdown reference in the UI object for later use
  ui.toggleSwitches[dropdownId] = {
      element: dropdownTrigger,
      options: dropdownMenu,
      toggle: () => this.toggleCustomDropdown(dropdownTrigger, dropdownMenu),
      state: () => dropdownMenu.classList.contains('hidden') ? null : dropdownTrigger.innerText,
      select: (option) => {
          dropdownTrigger.innerText = option.innerText;
          console.log(`Selected value: ${option.innerText}`);
          this.toggleCustomDropdown(dropdownTrigger, dropdownMenu); // Close the dropdown
      }
  };

  // Add event listener to toggle the dropdown
  dropdownTrigger.addEventListener('click', () => ui.toggleSwitches[dropdownId].toggle());

  console.log(`Custom dropdown '${dropdownId}' is now stored in ui.toggleSwitches.`);
},

toggleCustomDropdown: function(dropdownTrigger, dropdownMenu) {
    const isVisible = !dropdownMenu.classList.toggle('hidden'); // Toggle visibility
    this.dropdownOpen = isVisible; // Set dropdownOpen based on visibility

    if (isVisible) {
        dropdownTrigger.classList.add('dropdown-active');
        this.activeDropdown = dropdownMenu;

        // Check if there's a previously selected option
        const selectedValue = dropdownTrigger.innerText;
        const options = dropdownMenu.querySelectorAll('.dropdown-option');

        this.activeDropdownIndex = Array.from(options).findIndex(option => option.innerText === selectedValue);

        // If no match is found, default to the first option
        if (this.activeDropdownIndex === -1) {
            this.activeDropdownIndex = 0;
        }

        // Highlight the current option
        options.forEach((option, index) => {
            option.classList.toggle('bg-gray-600', index === this.activeDropdownIndex);
            option.classList.toggle('text-white', index === this.activeDropdownIndex);
        });
    } else {
        dropdownTrigger.classList.remove('dropdown-active');
        this.activeDropdown = null;
    }
},

highlightDropdownOption: function(direction) {
  if (this.activeDropdown) {
      const options = Array.from(this.activeDropdown.querySelectorAll('.dropdown-option'));
      this.activeDropdownIndex = (this.activeDropdownIndex + direction + options.length) % options.length;

      options.forEach((option, index) => {
          option.classList.toggle('bg-gray-600', index === this.activeDropdownIndex);
          option.classList.toggle('text-white', index === this.activeDropdownIndex);
      });
  }
},

confirmDropdownSelection: function() {
  if (this.activeDropdown) {
      const selectedOption = this.activeDropdown.querySelector('.dropdown-option.bg-gray-600');
      if (selectedOption) {
          console.log(`Selected option: ${selectedOption.innerText}`);
          const dropdownTrigger = this.activeDropdown.previousElementSibling;
          ui.toggleSwitches[dropdownTrigger.id].select(selectedOption); // Confirm the selection
      }
  }
},

cancelDropdownSelection: function() {
  if (this.activeDropdown) {
      this.toggleCustomDropdown(this.activeDropdown.previousElementSibling, this.activeDropdown); // Close the dropdown
  }
},

initAccordion: function(containerId) {
    const container = document.getElementById(containerId);
    if (!container) return;

    const accordionHeaders = container.querySelectorAll('.accordion-header');

    this.accordions[containerId] = { accordionHeaders };

    accordionHeaders.forEach(header => {
        header.addEventListener('click', () => {
            const content = header.nextElementSibling;
            if (content.classList.contains('hidden')) {
                // Slide down
                content.style.maxHeight = content.scrollHeight + 'px';
                content.classList.remove('hidden');
                content.classList.add('block');
            } else {
                // Slide up
                content.style.maxHeight = '0';
                setTimeout(() => {
                    content.classList.remove('block');
                    content.classList.add('hidden');
                }, 300);
            }
        });
    });
},

destroyAccordion: function(containerId) {
    const accordion = this.accordions[containerId];
    if (!accordion) return;

    accordion.accordionHeaders.forEach(header => {
        const newHeader = header.cloneNode(true);
        header.replaceWith(newHeader);
    });

    delete this.accordions[containerId];
},

contextMenu: {
  showContextMenu: function (menuElement, menuItemsElement, config, clientX, clientY) {
    // Clear existing items
    menuItemsElement.innerHTML = '';

    // Build menu
    this.buildMenu(menuItemsElement, config);

    // Temporarily unhide to measure
    menuElement.classList.remove('hidden');
    const w = menuElement.offsetWidth;
    const h = menuElement.offsetHeight;

    // Calculate final position to avoid main-menu overflow
    let finalLeft = clientX;
    let finalTop = clientY;

    // If menu goes out of the right edge, flip to left side
    if (clientX + w > window.innerWidth) {
      finalLeft = clientX - w;
      if (finalLeft < 0) finalLeft = 0; // clamp to screen
    }

    // If menu goes out of the bottom edge, flip to top side
    if (clientY + h > window.innerHeight) {
      finalTop = clientY - h;
      if (finalTop < 0) finalTop = 0; // clamp to screen
    }

    // Position the main menu
    menuElement.style.left = finalLeft + 'px';
    menuElement.style.top = finalTop + 'px';
  },

  buildMenu: function (parentUl, items) {
    items.forEach((item) => {
      let li = document.createElement('li');
      li.classList.add('px-4', 'py-2', 'cursor-pointer', 'hover:bg-gray-900', 'text-white');

      if (item.type === 'checkbox') {
        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.id = item.id;
        checkbox.checked = item.initialValue;
        li.style.userSelect = 'none';

        // Toggle the checkbox manually, do not hide menu
        li.addEventListener('click', () => {
          checkbox.checked = !checkbox.checked;
          item.initialValue = checkbox.checked;
          if (item.callback) item.callback(checkbox.checked);
        });

        li.appendChild(checkbox);
        li.appendChild(document.createTextNode(' ' + item.label));
      }
      else if (item.type === 'number') {
        // Number input
        li.textContent = item.label;

        const numberInput = document.createElement('input');
        numberInput.type = 'number';
        numberInput.id = item.id;
        numberInput.value = item.initialValue;
        numberInput.classList.add('ml-2', 'w-16', 'text-black', 'px-1', 'py-1', 'border', 'border-gray-600');

        // Keep menu open if user clicks in the input
        numberInput.addEventListener('click', (e) => e.stopPropagation());

        // Trigger callback on input
        numberInput.addEventListener('input', (e) => {
          item.initialValue = Number(e.target.value);
          if (item.callback) item.callback(Number(e.target.value));
        });

        li.appendChild(numberInput);
      }
      else if (item.subMenu) {
        // Nested submenus
        li.textContent = item.label;

        let arrow = document.createElement('span');
        arrow.textContent = '▶';
        arrow.classList.add('ml-2', 'text-gray-400');
        li.appendChild(arrow);

        li.classList.add('relative', 'group');

        let nestedUl = document.createElement('ul');
        nestedUl.classList.add(
          'hidden',
          'absolute',
          'bg-black',
          'rounded-lg',
          'shadow-lg',
          'z-50',
          'top-0',
          'text-white'
        );
        nestedUl.style.minWidth = '200px';

        // Recursively build the submenu
        this.buildMenu(nestedUl, item.subMenu);
        li.appendChild(nestedUl);

        // Show/hide with flipping logic
        li.addEventListener('mouseenter', () => {
          // Temporarily unhide to measure
          nestedUl.classList.remove('hidden');

          // Position sub-menu to the right by default
          nestedUl.style.left = li.offsetWidth + 'px';
          nestedUl.style.top = '0';

          // Measure
          let subW = nestedUl.offsetWidth;
          let subH = nestedUl.offsetHeight;

          let liRect = li.getBoundingClientRect();
          let rightEdge = liRect.left + liRect.width + subW;
          let bottomEdge = liRect.top + subH;

          // Flip horizontally if needed
          if (rightEdge > window.innerWidth) {
            // Position sub-menu to the left
            nestedUl.style.left = -subW + 'px';
          }

          // Flip vertically if needed
          let topVal = 0;
          if (bottomEdge > window.innerHeight) {
            // Move it up so it's fully visible
            topVal = -(subH - liRect.height);
          }
          nestedUl.style.top = topVal + 'px';
        });

        li.addEventListener('mouseleave', () => {
          nestedUl.classList.add('hidden');
        });
      }
      else {
        // Normal menu item
        li.textContent = item.label;
        if (item.callback) {
          li.onclick = (e) => item.callback(e.clientX, e.clientY);
        }
      }

      parentUl.appendChild(li);
    });

    // After building items, round the first and last item in the list
    const allLis = parentUl.querySelectorAll(':scope > li');
    if (allLis.length > 0) {
      allLis[0].classList.add('rounded-t-lg');
      allLis[allLis.length - 1].classList.add('rounded-b-lg');
    }
  },

  hideMenus: function (event, menuElement) {
    // If the click is outside the menu, hide it. 
    // If the click is inside the menu, do nothing (keep it open).
    if (!menuElement.contains(event.target)) {
      menuElement.classList.add('hidden');
    }
  },

  disableDefaultContextMenu: function (event, callback) {
    event.preventDefault();
    if (callback) callback(event.clientX, event.clientY);
  },
}

};