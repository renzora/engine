ui = {
  menus: {},
  activeMenuId: null,
  activeSubItemIndex: 0,

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