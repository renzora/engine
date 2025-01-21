window.pluginResolves = window.pluginResolves || {};

plugin = {
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
            if (!element) {
                return;
            }
        } else {
            element = elementOrId;
        }

        this.plugins = this.plugins.filter(plugin => plugin !== element);
        this.plugins.push(element);

        this.plugins.forEach((plugin, index) => {
            if (plugin) {
                try {
                    plugin.style.zIndex = (this.baseZIndex + index).toString();
                } catch (error) {
                    console.error('Error setting zIndex for plugin:', plugin, error);
                }
            } else {
                console.error('Undefined plugin in plugin list at index:', index);
            }
        });

        this.activePlugin = element.getAttribute('data-window');
    },

    initDraggable: function(element, options) {
        if (
            element.getAttribute('data-drag') === 'false' ||
            (options && options.drag === false)
        ) {
            return;
        }

        let isDragging = false;
        let originalX, originalY, startX, startY;

        const onStart = (e) => {
            const isTouchEvent = e.type === 'touchstart';
            const clientX = isTouchEvent ? e.touches[0].clientX : e.clientX;
            const clientY = isTouchEvent ? e.touches[0].clientY : e.clientY;

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

            if (isTouchEvent) {
                document.addEventListener('touchmove', onMove, { passive: false });
                document.addEventListener('touchend', onEnd);
            } else {
                document.addEventListener('mousemove', onMove);
                document.addEventListener('mouseup', onEnd);
            }
        };

        const onMove = (e) => {
            if (!isDragging) return;

            const isTouchEvent = e.type === 'touchmove';
            const clientX = isTouchEvent ? e.touches[0].clientX : e.clientX;
            const clientY = isTouchEvent ? e.touches[0].clientY : e.clientY;

            const dx = clientX - startX;
            const dy = clientY - startY;

            let newLeft = originalX + dx;
            let newTop = originalY + dy;

            const windowWidth = window.innerWidth;
            const windowHeight = window.innerHeight;
            const pluginRect = element.getBoundingClientRect();

            if (newLeft < 0) newLeft = 0;
            if (newTop < 0) newTop = 0;
            if (newLeft + pluginRect.width > windowWidth) {
                newLeft = windowWidth - pluginRect.width;
            }
            if (newTop + pluginRect.height > windowHeight) {
                newTop = windowHeight - pluginRect.height;
            }

            element.style.left = `${newLeft}px`;
            element.style.top = `${newTop}px`;

            if (options && typeof options.drag === 'function') {
                options.drag.call(element, e);
            }

            if (isTouchEvent) {
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

            const isTouchEvent = e.type === 'touchend';
            if (isTouchEvent) {
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
        const pluginElement = document.querySelector("[data-window='" + id + "']");
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

        const nextplugin = this.preloadQueue.shift();
        this.load(nextplugin.options).then(() => {
            this.loadNextPreload();
        });
    },

    load: function(options) {
        const {
            id = `plugin_${Date.now()}`,
            url,
            name = null,
            before = null,
            beforeStart = null,
            after = null,
            onError = null,
            drag = true,
            reload = false,
            hidden = false,
        } = options;

        const useApi = url.endsWith('.njk');
        const fullUrl = useApi ? `/api/ajax/${url}` : url;

        if (name) {
            this.pluginNames[id] = name;
        }

        return new Promise((resolve, reject) => {
            let existingPlugin = document.querySelector(`[data-window='${id}']`);

            if (existingPlugin) {
                if (reload) {
                    this.close(id);
                } else {
                    if (!hidden) {
                        this.front(existingPlugin);
                    } else {
                        this.minimize(id);
                    }
                    if (after && typeof after === 'function') {
                        after(id);
                    }
                    resolve();
                    return;
                }
            }

            if (before) {
                before(id);
            }

            if (useApi) {
                ui.ajax({
                    url: fullUrl,
                    method: 'GET',
                    success: (data) => this.processLoadedContent(data, id, url, beforeStart, after, hidden, resolve),
                    error: (error) => {
                        console.error(
                            `Failed to fetch plugin content for ID: '${id}' from URL: '${fullUrl}'`,
                            error
                        );
                        if (onError) {
                            onError(error, id);
                        }
                        reject(`Failed to load plugin '${id}' from URL: '${fullUrl}'`);
                    }
                });
            } else {
                fetch(fullUrl)
                    .then(response => {
                        if (!response.ok) {
                            throw new Error(`HTTP error! status: ${response.status}`);
                        }
                        return response.text();
                    })
                    .then(data => this.processLoadedContent(data, id, url, beforeStart, after, hidden, resolve))
                    .catch(error => {
                        console.error(
                            `Failed to fetch plugin content for ID: '${id}' from URL: '${fullUrl}'`,
                            error
                        );
                        if (onError) {
                            onError(error, id);
                        }
                        reject(`Failed to load plugin '${id}' from URL: '${fullUrl}'`);
                    });
            }
        });
    },

    processLoadedContent: function(data, id, url, beforeStart, after, hidden, resolve) {
        window.id = id;

        if (url.endsWith('.js')) {
            const script = document.createElement('script');
            script.textContent = data;
            script.setAttribute('id', `${id}_script`);
            document.head.appendChild(script);

            if (window[id]?.start && !window[id]._hasStarted) {
                window[id]._hasStarted = true;
                if (beforeStart) {
                    beforeStart(id);
                }
                window[id].start();
            }
        } else {
            const tempContainer = document.createElement('div');
            tempContainer.innerHTML = data;

            const topDiv = tempContainer.querySelector('div') || document.createElement('div');
            topDiv.setAttribute('data-window', id);

            const style = tempContainer.querySelector('style');
            if (style) {
                topDiv.prepend(style);
            }

            const lastPluginHtml = document.querySelector('div[data-window]:last-of-type');
            if (lastPluginHtml) {
                lastPluginHtml.after(topDiv);
            } else {
                document.body.appendChild(topDiv);
            }

            const script = tempContainer.querySelector('script');
            if (script) {
                const dynamicScript = document.createElement('script');
                dynamicScript.textContent = script.textContent;
                dynamicScript.setAttribute('id', `${id}_script`);
                document.head.appendChild(dynamicScript);

                if (!script.textContent.includes(`${id}.start()`) && window[id]?.start) {
                    if (beforeStart) {
                        beforeStart(id);
                    }
                    window[id].start();
                }
            }

            this.init(`[data-window='${id}']`, {
                start: function() { this.classList.add('dragging'); },
                drag: function() {},
                stop: function() { this.classList.remove('dragging'); },
            });

            if (hidden) {
                this.minimize(id);
            } else {
                this.front(id);
            }
        }

        if (window[id]) {
            this.loadedPlugins[id] = window[id];
        }

        if (after && typeof after === 'function') {
            after(id);
        }

        resolve();
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

    show: function(pluginId) {
        const p = document.querySelector("[data-window='" + pluginId + "']");
        if (p) {
            p.style.display = 'block';
            this.front(pluginId);
        }
    },

    close: function(id, fromEscKey = false) {
        const pluginElement = document.querySelector(`[data-window='${id}']`);
        if (pluginElement) {
            pluginElement.remove();
        }

        if (fromEscKey && pluginElement && pluginElement.getAttribute('data-close') === 'false') {
            return;
        }

        if (typeof window[id] !== 'undefined' && typeof window[id].unmount === 'function') {
            window[id].unmount();
            for (let key in window[id]) {
                if (Object.prototype.hasOwnProperty.call(window[id], key)) {
                    window[id][key] = null;
                }
            }
        }

        const styleElement = document.getElementById(id);
        if (styleElement) {
            styleElement.remove();
        }

        const scriptElement = document.querySelector(`script[id='${id}_script']`);
        if (scriptElement) {
            scriptElement.remove();
        }

        this.plugins = this.plugins.filter(plugin => plugin.getAttribute('data-window') !== id);

        if (this.loadedPlugins[id]) {
            delete this.loadedPlugins[id];
        }

        if (typeof window[id] !== 'undefined') {
            delete window[id];
        }

        if (this.plugins.length > 0) {
            const nextActivePlugin = this.plugins[this.plugins.length - 1];
            this.front(nextActivePlugin.getAttribute('data-window'));
        } else {
            this.activePlugin = null;
        }
    },

    unmount: function(id) {
        console.log("attempting to unmount", id);

        if (window[id] && typeof window[id].unmount === 'function') {
            window[id].unmount();
        }

        let obj = window[id];
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
            if (this.loadedPlugins[id]) {
                delete this.loadedPlugins[id];
            }
            console.log(id, "has been completely unmounted and deleted.");
        }
    },

    topZIndex: function() {
        const highestZIndex = Array.from(document.querySelectorAll('*'))
            .map(el => parseFloat(window.getComputedStyle(el).zIndex))
            .filter(zIndex => !isNaN(zIndex))
            .reduce((max, zIndex) => Math.max(max, zIndex), 0);

        return highestZIndex;
    },

    getActivePlugin: function() {
        return this.activePlugin;
    },

    showAll: function() {
        const plugins = document.querySelectorAll("[data-window]");
        plugins.forEach(function(plugin) {
            plugin.style.display = 'block';
        });

        if (this.plugins.length > 0) {
            const highestplugin = this.plugins[this.plugins.length - 1];
            this.front(highestplugin.getAttribute('data-window'));
        }
    },

    hideAll: function() {
        const plugins = document.querySelectorAll("[data-window]");
        plugins.forEach(function(plugin) {
            plugin.style.display = 'none';
        });

        this.activePlugin = null;
    },

    closeAll: function() {
        const windows = document.querySelectorAll('[data-window]');
        windows.forEach((windowElement) => {
            const id = windowElement.getAttribute('data-window');
            windowElement.remove();
            ui.unmount(id);
        });
    },

    closest: function(element) {
        while (element && !element.dataset.window) {
            element = element.parentElement;
        }
        return element ? element.dataset.window : null;
    },

    isVisible: function(id) {
        const pluginElement = document.querySelector("[data-window='" + id + "']");
        return pluginElement && pluginElement.style.display !== 'none';
    },

    exists: function(id) {
        return document.querySelector("[data-window='" + id + "']") !== null;
    }
};
