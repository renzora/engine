ui = {
  menus: {},
  activeMenuId: null,
  activeSubItemIndex: 0,

  html(selectorOrElement, htmlString, action = 'replace') {
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

    const tempContainer = document.createElement('div');
    tempContainer.innerHTML = htmlString;
    Array.from(tempContainer.querySelectorAll('script')).forEach(oldScript => {
        const newScript = document.createElement('script');
        if (oldScript.src) {
            newScript.src = oldScript.src;
            newScript.async = false;
        } else {
            newScript.textContent = oldScript.textContent;
        }
        Array.from(oldScript.attributes).forEach(attr => newScript.setAttribute(attr.name, attr.value));
        document.body.appendChild(newScript);
        document.body.removeChild(newScript);
    });
},

ajax: async function({ url, method = 'GET', data = null, outputType = 'text', success, error }) {
  try {
      let fetchUrl;

      if (url.startsWith('/api/ajax')) {
          fetchUrl = url;
      } else if (url.startsWith('/api/') && !url.startsWith('/api/ajax')) {
          fetchUrl = url;
      } else {
          fetchUrl = `/api/ajax/${url.replace(/^\/+/, '')}`;
      }

      const init = {
          method: method,
          headers: {},
      };

      if (data) {
          if (method === 'GET') {
              const queryParams = new URLSearchParams(data).toString();
              fetchUrl = `${fetchUrl}?${queryParams}`;
          } else {
              if (typeof data === 'object') {
                  init.headers['Content-Type'] = 'application/json';
                  init.body = JSON.stringify(data);
              } else {
                  init.headers['Content-Type'] = 'application/x-www-form-urlencoded';
                  init.body = data;
              }
          }
      }

      const response = await fetch(fetchUrl, init);

      if (!response.ok) {
          const errorText = await response.text();
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
      console.error('Failed to fetch data:', err);
      if (error) {
          if (err instanceof Error) {
              error(err.message);
          } else {
              error(err);
          }
      }
  }
},

isMobile() {
  const userAgent = navigator.userAgent || navigator.vendor || window.opera;

  if (/android/i.test(userAgent)) {
      return true;
  }
  if (/iPad|iPhone|iPod/.test(userAgent) && !window.MSStream) {
      return true;
  }

  return window.innerWidth <= 768;
},

fullScreen() {
  const element = document.documentElement;

  if (!document.fullscreenElement && !document.webkitFullscreenElement && !document.msFullscreenElement) {
      if (element.requestFullscreen) {
          element.requestFullscreen().catch((err) => {
              console.error(`Error attempting to enable fullscreen mode: ${err.message}`);
          });
      } else if (element.webkitRequestFullscreen) {
          element.webkitRequestFullscreen();
      } else if (element.msRequestFullscreen) {
          element.msRequestFullscreen();
      }

      if (/android/i.test(navigator.userAgent)) {
          document.addEventListener('fullscreenchange', () => {
              if (document.fullscreenElement) {
                  window.scrollTo(0, 1);
              }
          });
      }
  } else {
      if (document.exitFullscreen) {
          document.exitFullscreen().catch((err) => {
              console.error(`Error attempting to exit fullscreen mode: ${err.message}`);
          });
      } else if (document.webkitExitFullscreen) {
          document.webkitExitFullscreen();
      } else if (document.msExitFullscreen) {
          document.msExitFullscreen();
      }
  }
},

contextMenu: {
  showContextMenu: function (menuElement, menuItemsElement, config, clientX, clientY) {
    menuItemsElement.innerHTML = '';
    this.buildMenu(menuItemsElement, config);
    menuElement.classList.remove('hidden');
    const w = menuElement.offsetWidth;
    const h = menuElement.offsetHeight;
    let finalLeft = clientX;
    let finalTop = clientY;

    if(clientX + w > window.innerWidth) {
      finalLeft = clientX - w;
      if (finalLeft < 0) finalLeft = 0;
    }

    if (clientY + h > window.innerHeight) {
      finalTop = clientY - h;
      if (finalTop < 0) finalTop = 0;
    }

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

        li.addEventListener('click', () => {
          checkbox.checked = !checkbox.checked;
          item.initialValue = checkbox.checked;
          if (item.callback) item.callback(checkbox.checked);
        });

        li.appendChild(checkbox);
        li.appendChild(document.createTextNode(' ' + item.label));
      }
      else if (item.type === 'number') {
        li.textContent = item.label;

        const numberInput = document.createElement('input');
        numberInput.type = 'number';
        numberInput.id = item.id;
        numberInput.value = item.initialValue;
        numberInput.classList.add('ml-2', 'w-16', 'text-black', 'px-1', 'py-1', 'border', 'border-gray-600');
        numberInput.addEventListener('click', (e) => e.stopPropagation());

        numberInput.addEventListener('input', (e) => {
          item.initialValue = Number(e.target.value);
          if (item.callback) item.callback(Number(e.target.value));
        });

        li.appendChild(numberInput);
      }
      else if (item.subMenu) {

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
        this.buildMenu(nestedUl, item.subMenu);
        li.appendChild(nestedUl);

        li.addEventListener('mouseenter', () => {
          nestedUl.classList.remove('hidden');
          nestedUl.style.left = li.offsetWidth + 'px';
          nestedUl.style.top = '0';

          let subW = nestedUl.offsetWidth;
          let subH = nestedUl.offsetHeight;
          let liRect = li.getBoundingClientRect();
          let rightEdge = liRect.left + liRect.width + subW;
          let bottomEdge = liRect.top + subH;

          if (rightEdge > window.innerWidth) {
            nestedUl.style.left = -subW + 'px';
          }

          let topVal = 0;
          if (bottomEdge > window.innerHeight) {
            topVal = -(subH - liRect.height);
          }
          nestedUl.style.top = topVal + 'px';
        });

        li.addEventListener('mouseleave', () => {
          nestedUl.classList.add('hidden');
        });
      }
      else {
        li.textContent = item.label;
        if (item.callback) {
          li.onclick = (e) => item.callback(e.clientX, e.clientY);
        }
      }

      parentUl.appendChild(li);
    });

    const allLis = parentUl.querySelectorAll(':scope > li');
    if (allLis.length > 0) {
      allLis[0].classList.add('rounded-t-lg');
      allLis[allLis.length - 1].classList.add('rounded-b-lg');
    }
  },

  hideMenus: function (event, menuElement) {
    if (!menuElement.contains(event.target)) {
      menuElement.classList.add('hidden');
    }
  },

  disableDefaultContextMenu: function (event, callback) {
    event.preventDefault();
    if (callback) callback(event.clientX, event.clientY);
  }
},

unmount(id) {
  
  if (window[id] && typeof window[id].unmount === 'function') {
      window[id].unmount();
  }

  var obj = window[id];

  if (obj) {
      if (obj.eventListeners && Array.isArray(obj.eventListeners)) {
          obj.eventListeners.length = 0;
      }

      for (var prop in obj) {
          if (obj.hasOwnProperty(prop)) {
              if (typeof obj[prop] === "function") {
                  delete obj[prop];
              } else if (Array.isArray(obj[prop])) {
                  obj[prop] = [];
              } else if (typeof obj[prop] === "object" && obj[prop] !== null) {
                  obj[prop] = {};
              } else {
                  obj[prop] = null;
              }
          }
      }
      delete window[id];
  }
}

};