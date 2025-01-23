window.pluginResolves = window.pluginResolves || {};

const rawPlugin = {
  plugins: [],
  baseZIndex: null,
  pluginNames: {},
  activePlugin: null,
  preloadQueue: [],
  loadedPlugins: {},

  init: function(selector, options) {
    const element = document.querySelector(selector);
    if (!element) {
      return;
    }

    if (this.plugins.length === 0) {
      this.baseZIndex = this.topZIndex() + 1;
    }

    this.initDraggable(element, options);
    this.initCloseButton(element);

    const highestZIndex = this.baseZIndex + this.plugins.length;
    element.style.zIndex = highestZIndex.toString();
    this.plugins.push(element);

    element.addEventListener('click', () => this.front(element));
  },

  front: function(elementOrId) {
    let element;
    if (typeof elementOrId === 'string') {
      element = document.querySelector(`[data-window='${elementOrId}']`);
      if (!element) return;
    } else {
      element = elementOrId;
    }

    this.plugins = this.plugins.filter(plugin => plugin !== element);
    this.plugins.push(element);

    this.plugins.forEach((plugin, index) => {
      if (!plugin) {
        console.error('Undefined plugin in plugin list at index:', index);
        return;
      }
      try {
        plugin.style.zIndex = (this.baseZIndex + index).toString();
      } catch (error) {
        console.error('Error setting zIndex for plugin:', plugin, error);
      }
    });

    this.activePlugin = element.getAttribute('data-window');
  },

  initDraggable: function(element, options) {
    if (element.getAttribute('data-drag') === 'false' || (options && options.drag === false)) {
      return;
    }

    let isDragging = false;
    let originalX, originalY, startX, startY;

    const onStart = (e) => {
      const isTouch = e.type === 'touchstart';
      const clientX = isTouch ? e.touches[0].clientX : e.clientX;
      const clientY = isTouch ? e.touches[0].clientY : e.clientY;

      // Prevent dragging from body or resize handles
      if (e.target.closest('.window_body') || e.target.closest('.resize-handle')) {
        return;
      }

      isDragging = true;
      originalX = element.offsetLeft;
      originalY = element.offsetTop;
      startX = clientX;
      startY = clientY;

      if (options && typeof options.start === 'function') {
        options.start.call(element, e);
      }

      document.onselectstart = () => false;
      document.body.style.userSelect = 'none';

      this.front(element);

      if (isTouch) {
        document.addEventListener('touchmove', onMove, { passive: false });
        document.addEventListener('touchend', onEnd);
      } else {
        document.addEventListener('mousemove', onMove);
        document.addEventListener('mouseup', onEnd);
      }
    };

    const onMove = (e) => {
      if (!isDragging) return;

      const isTouch = e.type === 'touchmove';
      const clientX = isTouch ? e.touches[0].clientX : e.clientX;
      const clientY = isTouch ? e.touches[0].clientY : e.clientY;
      const dx = clientX - startX;
      const dy = clientY - startY;
      let newLeft = originalX + dx;
      let newTop = originalY + dy;
      const windowWidth = window.innerWidth;
      const windowHeight = window.innerHeight;
      const rect = element.getBoundingClientRect();

      if (newLeft < 0) newLeft = 0;
      if (newTop < 0) newTop = 0;
      if (newLeft + rect.width > windowWidth) {
        newLeft = windowWidth - rect.width;
      }
      if (newTop + rect.height > windowHeight) {
        newTop = windowHeight - rect.height;
      }

      element.style.left = `${newLeft}px`;
      element.style.top = `${newTop}px`;

      if (options && typeof options.drag === 'function') {
        options.drag.call(element, e);
      }

      if (isTouch) {
        e.preventDefault();
      }
    };

    const onEnd = (e) => {
      if (!isDragging) return;
      isDragging = false;

      document.onselectstart = null;
      document.body.style.userSelect = '';

      if (options && typeof options.stop === 'function') {
        options.stop.call(element, e);
      }

      const isTouch = e.type === 'touchend';
      if (isTouch) {
        document.removeEventListener('touchmove', onMove);
        document.removeEventListener('touchend', onEnd);
      } else {
        document.removeEventListener('mousemove', onMove);
        document.removeEventListener('mouseup', onEnd);
      }
    };

    element.addEventListener('mousedown', onStart);
    element.addEventListener('touchstart', onStart, { passive: false });
  },

  initCloseButton: function(element) {
    const closeButton = element.querySelector('[data-close]');
    if (closeButton) {
      closeButton.addEventListener('click', (e) => {
        e.stopPropagation();
        const pluginId = element.getAttribute('data-window');
        this.close(pluginId);
      });
    }
  },

  minimize: function(id) {
    const pluginElement = document.querySelector(`[data-window='${id}']`);
    if (pluginElement) {
      pluginElement.style.display = 'none';

      if (this.plugins.length > 0) {
        const nextActivePlugin = this.plugins[this.plugins.length - 1];
        this.front(nextActivePlugin.getAttribute('data-window'));
      } else {
        this.activePlugin = null;
      }
    }
  },

  preload: function(pluginList) {
    this.preloadQueue = pluginList.sort((a, b) => a.priority - b.priority);
    this.loadNextPreload();
  },

  loadNextPreload: function() {
    if (this.preloadQueue.length === 0) return;
    const nextPlugin = this.preloadQueue.shift();
    this.load(nextPlugin.id, nextPlugin).then(() => {
      this.loadNextPreload();
    });
  },

  load: function(
    id,
    {
      path = '',
      ext = 'js',
      reload = false,
      hidden = false,
      before = null,
      beforeStart = null,
      after = null,
      onError = null,
      drag = true,
    } = {}
  ) {
    const existingPlugin = document.querySelector(`[data-window='${id}']`);

    return new Promise((resolve, reject) => {
      if (existingPlugin) {
        if (reload) {
          this.close(id);
        } else {
          hidden ? this.minimize(id) : this.front(existingPlugin);
          if (after) after(id);
          resolve();
          return;
        }
      }

      if (before) before(id);

      // The important fix: path is now placed before the id.
      const finalDir = path ? `${path}/${id}` : id;

      let url;
      if (ext === 'njk') {
        url = `/api/ajax/plugins/${finalDir}/index.njk`;
      } else if (ext === 'html') {
        url = `plugins/${finalDir}/index.html`;
      } else {
        url = `plugins/${finalDir}/index.js`;
      }

      fetch(url)
        .then(response => {
          if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
          }
          return response.text();
        })
        .then(data => {
          this._processLoadedContent({
            id,
            data,
            ext,
            beforeStart,
            after,
            hidden,
            drag
          });
          resolve();
        })
        .catch(err => {
          console.error(`Failed to load plugin "${id}" from "${url}"`, err);
          if (onError) onError(err, id);
          reject(err);
        });
    });
  },

  _processLoadedContent: function({ id, data, ext, beforeStart, after, hidden, drag }) {
    window[id] = window[id] || {};
    window[id].id = id;

    if (ext === 'js') {
      const codeWithId = `
        window['${id}'] = window['${id}'] || {};
        window['${id}'].id = '${id}';
        ${data}
      `;
      const script = document.createElement('script');
      script.textContent = codeWithId;
      script.setAttribute('id', `${id}_script`);
      document.head.appendChild(script);

      if (window[id]?.start && !window[id]._hasStarted) {
        window[id]._hasStarted = true;
        if (beforeStart) beforeStart(id);
        window[id].start();
      }
    } else {
      const tempContainer = document.createElement('div');
      tempContainer.innerHTML = data;

      let topDiv = tempContainer.querySelector('div');
      if (!topDiv) {
        topDiv = document.createElement('div');
        topDiv.innerHTML = data;
      }
      topDiv.setAttribute('data-window', id);

      const styleTag = tempContainer.querySelector('style');
      if (styleTag) {
        topDiv.prepend(styleTag);
      }

      const last = document.querySelector('div[data-window]:last-of-type');
      if (last) {
        last.after(topDiv);
      } else {
        document.body.appendChild(topDiv);
      }

      const inlineScript = tempContainer.querySelector('script');
      if (inlineScript) {
        const codeWithId = `
          window['${id}'] = window['${id}'] || {};
          window['${id}'].id = '${id}';
          ${inlineScript.textContent}
        `;
        const dynamicScript = document.createElement('script');
        dynamicScript.textContent = codeWithId;
        dynamicScript.setAttribute('id', `${id}_script`);
        document.head.appendChild(dynamicScript);

        if (!inlineScript.textContent.includes(`${id}.start()`) && window[id]?.start) {
          if (beforeStart) beforeStart(id);
          window[id].start();
        }
      }

      this.init(`[data-window='${id}']`, {
        drag,
        start() { this.classList.add('dragging'); },
        stop() { this.classList.remove('dragging'); }
      });

      hidden ? this.minimize(id) : this.front(id);
    }

    if (window[id]) {
      this.loadedPlugins[id] = window[id];
    }

    if (after) after(id);
  },

  hook: function(hookName) {
    for (const pluginId in this.loadedPlugins) {
      const pluginObj = this.loadedPlugins[pluginId];
      if (pluginObj && typeof pluginObj[hookName] === 'function') {
        try {
          pluginObj[hookName]();
        } catch (err) {
          console.error(`Error running hook '${hookName}' for plugin '${pluginId}':`, err);
        }
      }
    }
  },

  show: function(id) {
    const el = document.querySelector(`[data-window='${id}']`);
    if (el) {
      el.style.display = 'block';
      this.front(id);
    }
  },

  close: function(id, fromEscKey = false) {
    const pluginEl = document.querySelector(`[data-window='${id}']`);
    if (pluginEl) {
      pluginEl.remove();
    }

    if (fromEscKey && pluginEl && pluginEl.getAttribute('data-close') === 'false') {
      return;
    }

    if (window[id]?.unmount) {
      window[id].unmount();
      for (let key in window[id]) {
        if (Object.prototype.hasOwnProperty.call(window[id], key)) {
          window[id][key] = null;
        }
      }
    }

    const styleEl = document.getElementById(id);
    if (styleEl) {
      styleEl.remove();
    }

    const scriptEl = document.querySelector(`script[id='${id}_script']`);
    if (scriptEl) {
      scriptEl.remove();
    }

    this.plugins = this.plugins.filter(p => p.getAttribute('data-window') !== id);

    if (this.loadedPlugins[id]) {
      delete this.loadedPlugins[id];
    }
    if (typeof window[id] !== 'undefined') {
      delete window[id];
    }

    if (this.plugins.length > 0) {
      const nextActive = this.plugins[this.plugins.length - 1];
      this.front(nextActive.getAttribute('data-window'));
    } else {
      this.activePlugin = null;
    }
  },

  unmount: function(id) {
    console.log("attempting to unmount", id);

    if (window[id]?.unmount) {
      window[id].unmount();
    }

    const obj = window[id];
    if (obj) {
      if (obj.eventListeners && Array.isArray(obj.eventListeners)) {
        obj.eventListeners.length = 0;
      }

      for (let prop in obj) {
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
      delete this.loadedPlugins[id];
      console.log(id, "has been completely unmounted and deleted.");
    }
  },

  topZIndex: function() {
    const highest = Array.from(document.querySelectorAll('*'))
      .map(el => parseFloat(window.getComputedStyle(el).zIndex))
      .filter(z => !isNaN(z))
      .reduce((max, z) => Math.max(max, z), 0);
    return highest;
  },

  getActivePlugin: function() {
    return this.activePlugin;
  },

  showAll: function() {
    const all = document.querySelectorAll("[data-window]");
    all.forEach(el => el.style.display = 'block');

    if (this.plugins.length > 0) {
      const topmost = this.plugins[this.plugins.length - 1];
      this.front(topmost.getAttribute('data-window'));
    }
  },

  hideAll: function() {
    const all = document.querySelectorAll("[data-window]");
    all.forEach(el => el.style.display = 'none');
    this.activePlugin = null;
  },

  closeAll: function() {
    const all = document.querySelectorAll('[data-window]');
    all.forEach(el => {
      const pluginId = el.getAttribute('data-window');
      el.remove();
      this.unmount(pluginId);
    });
  },

  closest: function(element) {
    while (element && !element.dataset.window) {
      element = element.parentElement;
    }
    return element ? element.dataset.window : null;
  },

  isVisible: function(id) {
    const el = document.querySelector(`[data-window='${id}']`);
    return el && el.style.display !== 'none';
  },

  exists: function(...objNames) {
    for (let objName of objNames) {
      try {
        if (typeof eval(objName) === 'undefined') {
          return false;
        }
      } catch (e) {
        return false;
      }
    }
    return true;
  }
};

plugin = new Proxy(rawPlugin, {
  get(target, propKey, receiver) {
    if (Reflect.has(target, propKey)) {
      return Reflect.get(target, propKey, receiver);
    }

    if (target.exists(propKey)) {
      return new Proxy(window[propKey], {
        get(subTarget, subProp, subReceiver) {
          if (Reflect.has(subTarget, subProp)) {
            return Reflect.get(subTarget, subProp, subReceiver);
          }
          return () => {};
        }
      });
    }

    return new Proxy({}, {
      get() {
        return () => {};
      }
    });
  }
});
