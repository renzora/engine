window.pluginResolves = window.pluginResolves || {};

const rawPlugin = {
    plugins: [],
    baseZIndex: null,
    activePlugin: null,
    preloadQueue: [],
    loadedPlugins: {},

    init(selector, options) {
        const element = document.querySelector(selector);
        if (!element) return;

        if (!this.plugins.length) this.baseZIndex = this.topZIndex() + 1;

        this.initDraggable(element, options);
        this.initCloseButton(element);

        element.style.zIndex = (this.baseZIndex + this.plugins.length).toString();
        this.plugins.push(element);
        element.addEventListener('click', () => this.front(element));
    },

    front(elementOrId) {
        const element = typeof elementOrId === 'string' 
            ? document.querySelector(`[data-window='${elementOrId}']`)
            : elementOrId;
        if (!element) return;

        this.plugins = this.plugins.filter(p => p !== element);
        this.plugins.push(element);
        this.plugins.forEach((p, i) => p.style.zIndex = (this.baseZIndex + i).toString());
        this.activePlugin = element.getAttribute('data-window');
    },

    initDraggable(element, options) {
        if (element.getAttribute('data-drag') === 'false' || options?.drag === false) return;

        let isDragging = false;
        let originalX, originalY, startX, startY;

        const handleDragStart = (e) => {
            if (e.target.closest('.window_body,.resize-handle')) return;
            
            const isTouch = e.type === 'touchstart';
            const coords = isTouch ? e.touches[0] : e;
            
            isDragging = true;
            originalX = element.offsetLeft;
            originalY = element.offsetTop;
            startX = coords.clientX;
            startY = coords.clientY;

            options?.start?.call(element, e);
            document.body.style.userSelect = 'none';
            this.front(element);

            const moveEvent = isTouch ? 'touchmove' : 'mousemove';
            const endEvent = isTouch ? 'touchend' : 'mouseup';
            input.assign(moveEvent, handleDragMove, { passive: !isTouch });
            input.assign(endEvent, handleDragEnd);
        };

        const handleDragMove = (e) => {
            if (!isDragging) return;

            const isTouch = e.type === 'touchmove';
            const coords = isTouch ? e.touches[0] : e;
            
            const newLeft = Math.max(0, Math.min(originalX + coords.clientX - startX, 
                                                window.innerWidth - element.offsetWidth));
            const newTop = Math.max(0, Math.min(originalY + coords.clientY - startY, 
                                               window.innerHeight - element.offsetHeight));

            element.style.left = `${newLeft}px`;
            element.style.top = `${newTop}px`;

            options?.drag?.call(element, e);
            if (isTouch) e.preventDefault();
        };

        const handleDragEnd = (e) => {
            if (!isDragging) return;
            isDragging = false;
            document.body.style.userSelect = '';
            options?.stop?.call(element, e);
            
            const isTouch = e.type === 'touchend';
            input.destroy(isTouch ? 'touchmove' : 'mousemove');
            input.destroy(isTouch ? 'touchend' : 'mouseup');
        };

        input.assign('mousedown', handleDragStart, {}, element);
        input.assign('touchstart', handleDragStart, { passive: false }, element);
    },

    initCloseButton(element) {
        const closeButton = element.querySelector('[data-close]');
        if (closeButton) {
            closeButton.addEventListener('click', e => {
                e.stopPropagation();
                this.close(element.getAttribute('data-window'));
            });
        }
    },

    minimize(id) {
        const element = document.querySelector(`[data-window='${id}']`);
        if (element) {
            element.style.display = 'none';
            if (this.plugins.length) {
                this.front(this.plugins[this.plugins.length - 1].getAttribute('data-window'));
            } else {
                this.activePlugin = null;
            }
        }
    },

    load(id, {
        path = '',
        ext = 'js',
        reload = false,
        hidden = false,
        before = null,
        beforeStart = null,
        after = null,
        onError = null,
        drag = true
    } = {}) {
        const existingPlugin = document.querySelector(`[data-window='${id}']`);

        return new Promise((resolve, reject) => {
            if (existingPlugin && !reload) {
                hidden ? this.minimize(id) : this.front(existingPlugin);
                after?.(id);
                resolve();
                return;
            }

            if (existingPlugin) this.close(id);
            before?.(id);

            const finalPath = path ? `${path}/${id}` : id;
            const url = ext === 'njk' ? `/api/ajax/plugins/${finalPath}/index.njk` :
                       ext === 'html' ? `plugins/${finalPath}/index.html` :
                       `plugins/${finalPath}/index.js`;

            fetch(url)
                .then(response => {
                    if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
                    return response.text();
                })
                .then(data => {
                    this._processLoadedContent({ id, data, ext, beforeStart, after, hidden, drag });
                    resolve();
                })
                .catch(err => {
                    console.error(`Failed to load plugin "${id}" from "${url}"`, err);
                    onError?.(err, id);
                    reject(err);
                });
        });
    },

    _processLoadedContent({ id, data, ext, beforeStart, after, hidden, drag }) {
        window[id] = window[id] || {};
        window[id].id = id;

        if (ext === 'js') {
            const script = document.createElement('script');
            script.textContent = data;
            script.id = `${id}_script`;
            document.head.appendChild(script);

            if (window[id]?.start && !window[id]._hasStarted) {
                window[id]._hasStarted = true;
                beforeStart?.(id);
                window[id].start();
            }
        } else {
            const container = document.createElement('div');
            container.innerHTML = data;

            const topDiv = container.querySelector('div') || container;
            topDiv.setAttribute('data-window', id);

            const styleTag = container.querySelector('style');
            if (styleTag) topDiv.prepend(styleTag);

            const last = document.querySelector('div[data-window]:last-of-type');
            last ? last.after(topDiv) : document.body.appendChild(topDiv);

            const script = container.querySelector('script');
            if (script) {
                const newScript = document.createElement('script');
                newScript.textContent = script.textContent;
                newScript.id = `${id}_script`;
                document.head.appendChild(newScript);

                if (!script.textContent.includes(`${id}.start()`) && window[id]?.start) {
                    beforeStart?.(id);
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

        if (window[id]) this.loadedPlugins[id] = window[id];
        after?.(id);
    },

    close(id, fromEscKey = false) {
        const element = document.querySelector(`[data-window='${id}']`);
        if (!element) return;

        if (fromEscKey && element.getAttribute('data-close') === 'false') return;

        element.remove();

        if (window[id]?.unmount) {
            window[id].unmount();
            Object.keys(window[id]).forEach(key => window[id][key] = null);
        }

        document.getElementById(id)?.remove();
        document.querySelector(`script[id='${id}_script']`)?.remove();

        this.plugins = this.plugins.filter(p => p.getAttribute('data-window') !== id);
        delete this.loadedPlugins[id];
        delete window[id];

        if (this.plugins.length) {
            this.front(this.plugins[this.plugins.length - 1].getAttribute('data-window'));
        } else {
            this.activePlugin = null;
        }
    },

    unmount(id) {
        if (window[id]?.unmount) window[id].unmount();

        const obj = window[id];
        if (obj) {
            if (Array.isArray(obj.eventListeners)) obj.eventListeners.length = 0;

            Object.keys(obj).forEach(key => {
                if (typeof obj[key] === 'function') delete obj[key];
                else if (Array.isArray(obj[key])) obj[key] = [];
                else if (typeof obj[key] === 'object' && obj[key]) obj[key] = {};
                else obj[key] = null;
            });

            delete window[id];
            delete this.loadedPlugins[id];
        }
    },

    preload(pluginList) {
        this.preloadQueue = pluginList;
        this.loadNextPreload();
    },

    loadNextPreload() {
        if (!this.preloadQueue.length) return;
        const next = this.preloadQueue.shift();
        this.load(next.id, next).then(() => this.loadNextPreload());
    },

    hook(hookName) {
        Object.entries(this.loadedPlugins).forEach(([id, plugin]) => {
            if (typeof plugin[hookName] === 'function') {
                try {
                    plugin[hookName]();
                } catch (err) {
                    console.error(`Error running hook '${hookName}' for plugin '${id}':`, err);
                }
            }
        });
    },

    topZIndex() {
        return Array.from(document.querySelectorAll('*'))
            .map(el => parseFloat(window.getComputedStyle(el).zIndex))
            .filter(z => !isNaN(z))
            .reduce((max, z) => Math.max(max, z), 0);
    },

    getActivePlugin() {
        return this.activePlugin;
    },

    show(id) {
        const el = document.querySelector(`[data-window='${id}']`);
        if (el) {
            el.style.display = 'block';
            this.front(id);
        }
    },

    showAll() {
        const elements = document.querySelectorAll('[data-window]');
        elements.forEach(el => el.style.display = 'block');
        if (this.plugins.length) {
            this.front(this.plugins[this.plugins.length - 1].getAttribute('data-window'));
        }
    },

    hideAll() {
        document.querySelectorAll('[data-window]').forEach(el => el.style.display = 'none');
        this.activePlugin = null;
    },

    closeAll() {
        document.querySelectorAll('[data-window]').forEach(el => {
            const id = el.getAttribute('data-window');
            el.remove();
            this.unmount(id);
        });
    },

    closest(element) {
        while (element && !element.dataset.window) {
            element = element.parentElement;
        }
        return element?.dataset.window || null;
    },

    isVisible(id) {
        const el = document.querySelector(`[data-window='${id}']`);
        return el && el.style.display !== 'none';
    },

    exists(...objNames) {
        return objNames.every(name => {
            try {
                return typeof eval(name) !== 'undefined';
            } catch {
                return false;
            }
        });
    }
};

plugin = new Proxy(rawPlugin, {
    get(target, propKey, receiver) {
        if (Reflect.has(target, propKey)) return Reflect.get(target, propKey, receiver);
        
        if (target.exists(propKey)) {
            return new Proxy(window[propKey], {
                get(subTarget, subProp) {
                    return Reflect.has(subTarget, subProp) 
                        ? Reflect.get(subTarget, subProp) 
                        : () => {};
                }
            });
        }
        
        return new Proxy({}, { get: () => () => {} });
    }
});