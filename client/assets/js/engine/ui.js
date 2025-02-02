ui = {
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
