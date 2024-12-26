window.modalResolves = window.modalResolves || {};

var modal = {
    modals: [],
    baseZIndex: null,
    modalNames: {},
    activeModal: null,
    preloadQueue: [],

    init: function(selector, options) {
        const element = document.querySelector(selector);
        if (!element) {
            console.error(`No element found for selector: ${selector}`);
            return;
        }

        if (this.modals.length === 0) {
            this.baseZIndex = this.topZIndex() + 1;
        }

        this.initDraggable(element, options);
        this.initCloseButton(element);

        const highestZIndex = this.baseZIndex + this.modals.length;
        element.style.zIndex = highestZIndex.toString();
        this.modals.push(element);
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
    
        this.modals = this.modals.filter(modal => modal !== element);
        this.modals.push(element);
    
        this.modals.forEach((modal, index) => {
            if (modal) {
                try {
                    modal.style.zIndex = (this.baseZIndex + index).toString();
                } catch (error) {
                    console.error('Error setting zIndex for modal:', modal, error);
                }
            } else {
                console.error('Undefined modal in modal list at index:', index);
            }
        });
    
        this.activeModal = element.getAttribute('data-window');
        console.log('Active Modal Set:', this.activeModal);
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
            const modalRect = element.getBoundingClientRect();
    
            // Constrain within viewport
            if (newLeft < 0) newLeft = 0;
            if (newTop < 0) newTop = 0;
            if (newLeft + modalRect.width > windowWidth) newLeft = windowWidth - modalRect.width;
            if (newTop + modalRect.height > windowHeight) newTop = windowHeight - modalRect.height;
    
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
                const modalId = element.getAttribute('data-window');
                this.close(modalId);
            });
        }
    },

    minimize: function(id) {
        var modalElement = document.querySelector("[data-window='" + id + "']");
        if (modalElement) {
            modalElement.style.display = 'none';

            if (this.modals.length > 0) {
                const nextActiveModal = this.modals[this.modals.length - 1];
                this.front(nextActiveModal.getAttribute('data-window'));
            } else {
                this.activeModal = null;
            }
        }
    },

    preload: function(modalList) {
        // modalList = [{ id, url, options, priority }, ...]
        this.preloadQueue = modalList.sort((a, b) => a.priority - b.priority);
        this.loadNextPreload();
    },

    loadNextPreload: function() {
        if (this.preloadQueue.length === 0) return;

        const nextModal = this.preloadQueue.shift();
        this.load(nextModal.options).then(() => {
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
            this.modalNames[id] = name;
        }

        return new Promise((resolve, reject) => {
            let existingModal = document.querySelector("[data-window='" + id + "']");

            if (onBeforeLoad && typeof onBeforeLoad === 'function') {
                onBeforeLoad(id);
            }

            if (existingModal) {
                if (reload) {
                    this.close(id);
                } else {
                    if (!hidden) {
                        this.front(existingModal);
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
                url: 'modals/' + url,
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

                    window.modalResolves[id] = resolve;

                    if (onAfterLoad && typeof onAfterLoad === 'function') {
                        onAfterLoad(id);
                    }

                    resolve();
                },
                error: (error) => {
                    console.error('Error loading modal:', id, error);
                    if (onError && typeof onError === 'function') {
                        onError(error, id);
                    }
                    reject(`Failed to load modal from ${url}: ${error}`);
                },
            });
        });
    },

    show: function(modalId) {
        var modal = document.querySelector("[data-window='" + modalId + "']");
        if (modal) {
            modal.style.display = 'block';
            this.front(modalId);
        }
    },

    close: function(id, fromEscKey = false) {
        var modalElement = document.querySelector("[data-window='" + id + "']");
        if (modalElement) {
            if (fromEscKey && modalElement.getAttribute('data-close') === 'false') {
                console.log(`Closing prevented for modal: ${id}`);
                return;
            }

            modalElement.remove();
            audio.playAudio("closeModal", assets.use('closeModal'), 'sfx');
            this.modals = this.modals.filter(modal => modal.getAttribute('data-window') !== id);
            ui.unmount(id);

            if (window.modalResolves && window.modalResolves[id]) {
                console.log("resolving and removing", window.modalResolves[id]);
                window.modalResolves[id]();
                delete window.modalResolves[id];
            }

            if (this.modals.length > 0) {
                const nextActiveModal = this.modals[this.modals.length - 1];
                this.front(nextActiveModal.getAttribute('data-window'));
            } else {
                this.activeModal = null;
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

    getActiveModal: function() {
        return this.activeModal;
    },

    showAll: function() {
        var modals = document.querySelectorAll("[data-window]");
        modals.forEach(function(modal) {
            modal.style.display = 'block';
        });
    
        if (this.modals.length > 0) {
            const highestModal = this.modals[this.modals.length - 1];
            this.front(highestModal.getAttribute('data-window'));
        }
    },

    hideAll: function() {
        var modals = document.querySelectorAll("[data-window]");
        modals.forEach(function(modal) {
            modal.style.display = 'none';
        });
    
        this.activeModal = null;
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
        var modalElement = document.querySelector("[data-window='" + id + "']");
        return modalElement && modalElement.style.display !== 'none';
    },

    exists: function(id) {
        return document.querySelector("[data-window='" + id + "']") !== null;
    }
};