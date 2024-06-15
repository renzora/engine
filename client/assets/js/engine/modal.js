window.modalResolves = window.modalResolves || {};

var modal = {
    modals: [],
    baseZIndex: null,
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
    },

    initDraggable: function(element, options) {
        let isDragging = false;
        let originalX, originalY, mouseX, mouseY;

        const onMouseDown = (e) => {
            if (e.target.closest('.window_body') || e.target.closest('.resize-handle')) {
                return;
            }

            isDragging = true;
            originalX = element.offsetLeft;
            originalY = element.offsetTop;

            mouseX = e.clientX;
            mouseY = e.clientY;

            if (options && typeof options.start === 'function') {
                options.start.call(element, e);
            }

            document.onselectstart = () => false;
            document.body.style.userSelect = 'none';
            this.front(element);
            document.addEventListener('mousemove', onMouseMove);
            document.addEventListener('mouseup', onMouseUp);
        };

        const onMouseMove = (e) => {
            if (!isDragging) return;

            const dx = e.clientX - mouseX;
            const dy = e.clientY - mouseY;

            let newLeft = originalX + dx;
            let newTop = originalY + dy;

            const windowWidth = window.innerWidth;
            const windowHeight = window.innerHeight;
            const modalRect = element.getBoundingClientRect();

            if (newLeft < 0) newLeft = 0;
            if (newTop < 0) newTop = 0;
            if (newLeft + modalRect.width > windowWidth) newLeft = windowWidth - modalRect.width;
            if (newTop + modalRect.height > windowHeight) newTop = windowHeight - modalRect.height;

            element.style.left = `${newLeft}px`;
            element.style.top = `${newTop}px`;

            if (options && typeof options.drag === 'function') {
                options.drag.call(element, e);
            }
        };

        const onMouseUp = (e) => {
            if (!isDragging) return;
            isDragging = false;

            document.onselectstart = null;
            document.body.style.userSelect = '';

            if (options && typeof options.stop === 'function') {
                options.stop.call(element, e);
            }

            document.removeEventListener('mousemove', onMouseMove);
            document.removeEventListener('mouseup', onMouseUp);
        };

        element.addEventListener('mousedown', onMouseDown);
    },

    load: function(page, window_name) {
        if (!page.includes('/')) {
            page += '/index.php';
        }

        if (!window_name) {
            const pageName = page.split('/')[0];
            window_name = `${pageName}_window`;
        }

        return new Promise((resolve, reject) => {
            let existingModal = document.querySelector("[data-window='" + window_name + "']");
            if (existingModal) {
                //this.show(window_name);
                this.front(existingModal);
            } else {
                ui.ajax({
                    url: 'modals/' + page,
                    method: 'GET',
                    success: (data) => {
                        ui.html(document.body, data, 'append');

                        this.init("[data-window='" + window_name + "']", {
                            start: function() {
                                this.classList.add('dragging');
                            },
                            drag: function() {},
                            stop: function() {
                                this.classList.remove('dragging');
                            }
                        });

                        window.modalResolves[window_name] = resolve;
                    }
                });
            }
        });
    },

    topZIndex: function() {
        const highestZIndex = Array.from(document.querySelectorAll('*'))
            .map(el => parseFloat(window.getComputedStyle(el).zIndex))
            .filter(zIndex => !isNaN(zIndex))
            .reduce((max, zIndex) => Math.max(max, zIndex), 0);

        return highestZIndex;
    },

    show: function(modalId) {
        var modal = document.querySelector("[data-window='" + modalId + "']");
        if (modal && modal.style.display === 'none') {
            modal.style.display = 'block';
        }
    },

    hide: function(modalId) {
        var modal = document.querySelector("[data-window='" + modalId + "']");
        if (modal) {
            modal.style.display = 'none';
        }
    },

    exists: function(modalId) {
        var modal = document.querySelector("[data-window='" + modalId + "']");
        return modal !== null;
    },

    close: function(id, fromEscKey = false) {
        var modalElement = document.querySelector("[data-window='" + id + "']");
        if (modalElement) {
            if (fromEscKey && modalElement.getAttribute('data-close') === 'false') {
                console.log(`Closing prevented for modal: ${id}`);
                return;
            }
    
            modalElement.remove();
            this.modals = this.modals.filter(modal => modal.getAttribute('data-window') !== id);
            ui.unmount(id);
    
            if (window.modalResolves && window.modalResolves[id]) {
                console.log("resolving and removing", window.modalResolves[id]);
                window.modalResolves[id]();
                delete window.modalResolves[id];
            }
        }
    },

    showAll: function() {
        var modals = document.querySelectorAll("[data-window]");
        modals.forEach(function(modal) {
            modal.style.display = 'block';
        });
    },

    hideAll: function() {
        var modals = document.querySelectorAll("[data-window]");
        modals.forEach(function(modal) {
            modal.style.display = 'none';
        });
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
    }
};