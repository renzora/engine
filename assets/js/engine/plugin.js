window.pluginResolves = window.pluginResolves || {};

var plugin = {
    plugins: [],
    baseZIndex: null,
    pluginNames: {},
    activePlugin: null,
    preloadQueue: [],

    init: function(selector, options) {
        const element = document.querySelector(selector);
        if (!element) {
            console.error(`No element found for selector: ${selector}`);
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
                console.error(`No element found for window id: ${elementOrId}`);
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
        console.log('Active plugin Set:', this.activePlugin);
    },

    initDraggable: function(element, options) {
        if (element.getAttribute('data-drag') === 'false' || (options && options.drag === false)) {
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
    
            // Constrain within viewport
            if (newLeft < 0) newLeft = 0;
            if (newTop < 0) newTop = 0;
            if (newLeft + pluginRect.width > windowWidth) newLeft = windowWidth - pluginRect.width;
            if (newTop + pluginRect.height > windowHeight) newTop = windowHeight - pluginRect.height;
    
            element.style.left = `${newLeft}px`;
            element.style.top = `${newTop}px`;
    
            if (options && typeof options.drag === 'function') {
                options.drag.call(element, e);
            }
    
            if (isTouchEvent) {
                e.preventDefault(); // Prevent scrolling while dragging
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
        var pluginElement = document.querySelector("[data-window='" + id + "']");
        if (pluginElement) {
            pluginElement.style.display = 'none';

            if (this.plugins.length > 0) {
                const nextactivePlugin = this.plugins[this.plugins.length - 1];
                this.front(nextactivePlugin.getAttribute('data-window'));
            } else {
                this.activePlugin = null;
            }
        }
    },

    preload: function(pluginList) {
        // pluginList = [{ id, url, options, priority }, ...]
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
            id,
            url,
            name = null,
            showInList = true,
            drag = true,
            reload = false,
            hidden = false,
            onBeforeLoad = null,
            onAfterLoad = null,
            onError = null,
        } = options;

        if (!url.includes('/')) {
            options.url += '/index.php';
        }

        if (name) {
            this.pluginNames[id] = name;
        }

        return new Promise((resolve, reject) => {
            let existingplugin = document.querySelector("[data-window='" + id + "']");

            if (onBeforeLoad && typeof onBeforeLoad === 'function') {
                onBeforeLoad(id);
            }

            if (existingplugin) {
                if (reload) {
                    this.close(id);
                } else {
                    if (!hidden) {
                        this.front(existingplugin);
                    } else {
                        this.minimize(id);
                    }
                    if (onAfterLoad && typeof onAfterLoad === 'function') {
                        onAfterLoad(id);
                    }
                    resolve();
                    return;
                }
            }

            ui.ajax({
                url: 'plugins/' + url,
                method: 'GET',
                success: (data) => {
                    ui.html(document.body, data, 'append');

                    this.init(`[data-window='${id}']`, {
                        start: function () {
                            this.classList.add('dragging');
                        },
                        drag: function () {},
                        stop: function () {
                            this.classList.remove('dragging');
                        },
                        drag,
                    });

                    if (hidden) {
                        this.minimize(id);
                    } else {
                        this.front(id);
                    }

                    window.pluginResolves[id] = resolve;

                    if (onAfterLoad && typeof onAfterLoad === 'function') {
                        onAfterLoad(id);
                    }

                    resolve();
                },
                error: (error) => {
                    console.error('Error loading plugin:', id, error);
                    if (onError && typeof onError === 'function') {
                        onError(error, id);
                    }
                    reject(`Failed to load plugin from ${url}: ${error}`);
                },
            });
        });
    },

    show: function(pluginId) {
        var plugin = document.querySelector("[data-window='" + pluginId + "']");
        if (plugin) {
            plugin.style.display = 'block';
            this.front(pluginId);
        }
    },

    close: function(id, fromEscKey = false) {
        var pluginElement = document.querySelector("[data-window='" + id + "']");
        if (pluginElement) {
            if (fromEscKey && pluginElement.getAttribute('data-close') === 'false') {
                console.log(`Closing prevented for plugin: ${id}`);
                return;
            }

            pluginElement.remove();
            audio.playAudio("closeplugin", assets.use('closeplugin'), 'sfx');
            this.plugins = this.plugins.filter(plugin => plugin.getAttribute('data-window') !== id);
            ui.unmount(id);

            if (window.pluginResolves && window.pluginResolves[id]) {
                console.log("resolving and removing", window.pluginResolves[id]);
                window.pluginResolves[id]();
                delete window.pluginResolves[id];
            }

            if (this.plugins.length > 0) {
                const nextactivePlugin = this.plugins[this.plugins.length - 1];
                this.front(nextactivePlugin.getAttribute('data-window'));
            } else {
                this.activePlugin = null;
            }
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
        var plugins = document.querySelectorAll("[data-window]");
        plugins.forEach(function(plugin) {
            plugin.style.display = 'block';
        });
    
        if (this.plugins.length > 0) {
            const highestplugin = this.plugins[this.plugins.length - 1];
            this.front(highestplugin.getAttribute('data-window'));
        }
    },

    hideAll: function() {
        var plugins = document.querySelectorAll("[data-window]");
        plugins.forEach(function(plugin) {
            plugin.style.display = 'none';
        });
    
        this.activePlugin = null;
    },

    closeAll: function() {
        var windows = document.querySelectorAll('[data-window]');
        windows.forEach(function(windowElement) {
            var id = windowElement.getAttribute('data-window');
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
        var pluginElement = document.querySelector("[data-window='" + id + "']");
        return pluginElement && pluginElement.style.display !== 'none';
    },

    exists: function(id) {
        return document.querySelector("[data-window='" + id + "']") !== null;
    }
};