window.pluginResolves = window.pluginResolves || {};

plugin = {
    plugins: [],
    baseZIndex: null,
    pluginNames: {},
    activePlugin: null,
    preloadQueue: [],

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

    load: function (options) {
        const {
            id = `plugin_${Date.now()}`, // Generate unique ID if not provided
            url, // URL for the plugin content
            name = null,
            onBeforeLoad = null,
            onAfterLoad = null,
            onError = null,
            drag = true,
            reload = false,
            hidden = false,
        } = options;
    
        const fullUrl = `/plugins/${url}`; // Ensure the URL is prefixed with /plugins/

        if (name) {
            this.pluginNames[id] = name;
        }
    
        return new Promise((resolve, reject) => {

            let existingplugin = document.querySelector("[data-window='" + id + "']");

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
    
            // Call onBeforeLoad if provided
            if (onBeforeLoad) {
                onBeforeLoad(id);
            }
    
            ui.ajax({
                url: fullUrl,
                method: 'GET',
                success: (data) => {
                    console.log(`Successfully fetched plugin content for ID: '${id}'`);

                    window.id = id;
                
                    // Create a temporary container for parsing the HTML
                    const tempContainer = document.createElement('div');
                    tempContainer.innerHTML = data;
                
                    // Find the top-level div (plugin HTML)
                    const topDiv = tempContainer.querySelector('div');
                    if (topDiv) {
                        topDiv.setAttribute('data-window', id); // Add data-window attribute
                
                        // Find and process the style
                        const style = tempContainer.querySelector('style');
                        if (style) {
                            topDiv.prepend(style); // Append style to the top of the plugin's HTML
                        }
                
                        const lastPluginHtml = document.querySelector('div[data-window]:last-of-type');
                        if (lastPluginHtml) {
                            lastPluginHtml.after(topDiv); // Append after the last plugin HTML
                        } else {
                            document.body.appendChild(topDiv); // Append to the body if none exist
                        }
                
                        // Set the active plugin
                        this.activePlugin = id;
                    }
                
                    // Check and execute the script even if no HTML is rendered
                    const script = tempContainer.querySelector('script');
                    if (script) {
                        const dynamicScript = document.createElement('script');
                        dynamicScript.textContent = script.textContent; // Copy script content
                        dynamicScript.setAttribute('id', `${id}_script`); // Append _script to the ID
                        const lastScript = document.querySelector('script:last-of-type');
                        if (lastScript) {
                            lastScript.after(dynamicScript); // Append after the last <script>
                        } else {
                            document.body.appendChild(dynamicScript); // Append to the body if none exist
                        }
                
                        // Automatically start the plugin only if start is not explicitly called in the script
                        if (!script.textContent.includes(`${id}.start()`) && window[id]?.start) {
                            window[id].start();
                        }
                    } else {
                        // Attempt to auto-start even if there’s no script
                        if (window[id]?.start && !window[id]._hasStarted) {
                            window[id]._hasStarted = true; // Flag to prevent multiple starts
                            window[id].start();
                        }
                    }
                
                    // Initialize drag/drop and visibility
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
                    console.error(`Failed to fetch plugin content for ID: '${id}' from URL: '${fullUrl}'`, error);
    
                    // Call onError if provided
                    if (onError) {
                        console.log(`Executing onError callback for plugin ID: '${id}'`);
                        onError(error, id);
                    }
    
                    // Reject the promise
                    reject(`Failed to load plugin '${id}' from URL: '${fullUrl}'`);
                }
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

    close: function (id, fromEscKey = false) {
        console.log(`Attempting to close plugin with ID: '${id}'`);
    
        const pluginElement = document.querySelector(`[data-window='${id}']`);
        if (pluginElement) {
            pluginElement.remove();
        }
    
        if (fromEscKey && pluginElement.getAttribute('data-close') === 'false') {
            return;
        }
    
        if (typeof window[id] !== 'undefined' && typeof window[id].unmount === 'function') {
            console.log(`Executing unmount method for plugin ID: '${id}'`);
            window[id].unmount();
    
            // Break internal references to facilitate garbage collection
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
    
        if (typeof window[id] !== 'undefined') {
            console.log(`Deleting global scope for plugin ID: '${id}'`);
            delete window[id];
        }
    
        if (this.plugins.length > 0) {
            const nextActivePlugin = this.plugins[this.plugins.length - 1];
            this.front(nextActivePlugin.getAttribute('data-window'));
        } else {
            this.activePlugin = null;
        }
    
        console.log(`Plugin with ID: '${id}' successfully closed.`);
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