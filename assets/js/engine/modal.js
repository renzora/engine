window.modalResolves = window.modalResolves || {};

var modal = {
    modals: [],
    baseZIndex: null,
    modalNames: {}, // Stores the display names of the modals
    showInListFlags: {}, // Stores the flags for showing modals in the list

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
        this.initMinimizeAndCloseButtons(element);

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

    initMinimizeAndCloseButtons: function(element) {
        const closeButton = element.querySelector('[data-close]');
        if (closeButton) {
            closeButton.addEventListener('click', (e) => {
                e.stopPropagation();
                const modalId = element.getAttribute('data-window');
                this.close(modalId);
            });
        }

        const minimizeButton = element.querySelector('[data-minimize]');
        if (minimizeButton) {
            minimizeButton.addEventListener('click', (e) => {
                e.stopPropagation();
                const modalId = element.getAttribute('data-window');
                this.minimize(modalId);
            });
        }
    },

    minimize: function(id) {
        var modalElement = document.querySelector("[data-window='" + id + "']");
        if (modalElement) {
            modalElement.style.display = 'none';
            // Optionally, you can add logic to show the minimized state or a taskbar icon
        }
    },

    load: function(page, window_name, modalName = null, showInList = true) {
        if (!page.includes('/')) {
            page += '/index.php';
        }

        if (!window_name) {
            const pageName = page.split('/')[0];
            window_name = `${pageName}_window`;
        }

        if (modalName) {
            this.modalNames[window_name] = modalName;
        }

        this.showInListFlags[window_name] = showInList;

        return new Promise((resolve, reject) => {
            let existingModal = document.querySelector("[data-window='" + window_name + "']");
            if (existingModal) {
                this.front(existingModal);
                resolve();
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
                        resolve();
                    },
                    error: (error) => {
                        console.error('Error loading modal:', window_name, error);
                        reject(`Failed to load modal from ${page}: ${error}`);
                    }
                });
            }
        });
    },

    updateModalsButtonVisibility: function() {
        const modalsButton = document.getElementById('show_modals_button');
        const modalsList = document.getElementById('modals_list');

        const visibleModals = this.modals.filter(modal => {
            const modalName = modal.getAttribute('data-window');
            return this.showInListFlags[modalName];
        });

        if (visibleModals.length > 0) {
            modalsButton.classList.remove('hidden');
            if (modalsList) {
                modalsList.classList.remove('hidden');
            }
        } else {
            modalsButton.classList.add('hidden');
            if (modalsList) {
                modalsList.classList.add('hidden');
            }
        }
    },

    showModalsList: function() {
        const modalsList = document.getElementById('modals_list');
        modalsList.innerHTML = '';

        const visibleModals = this.modals.filter(modal => {
            const modalName = modal.getAttribute('data-window');
            return this.showInListFlags[modalName];
        });

        if (visibleModals.length > 0) {
            visibleModals.forEach(modal => {
                const modalName = modal.getAttribute('data-window');
                const displayName = this.modalNames[modalName] || modalName;
                const modalItem = document.createElement('div');
                modalItem.classList.add('relative', 'flex', 'items-center', 'p-2', 'hover:bg-gray-700', 'rounded-md', 'cursor-pointer', 'text-white', 'overflow-hidden', 'w-full');

                const modalText = document.createElement('div');
                modalText.textContent = displayName;
                modalText.classList.add('flex-grow', 'truncate', 'text-white'); // Ensure text stays on one line and is truncated if too long

                const closeButton = document.createElement('button');
                closeButton.classList.add('icon', 'close_dark', 'absolute', 'right-0', 'hint--left', 'text-white', 'hover:text-red-500', 'hidden');
                closeButton.setAttribute('aria-label', 'Close');
                closeButton.addEventListener('click', (e) => {
                    e.stopPropagation();
                    this.close(modalName);
                    modalsList.removeChild(modalItem);
                    // Check if there are any items left
                    if (modalsList.children.length === 0) {
                        modalsList.classList.add('hidden');
                        this.updateModalsButtonVisibility();
                    }
                });

                modalItem.addEventListener('mouseover', () => {
                    closeButton.classList.remove('hidden'); // Show on hover
                });

                modalItem.addEventListener('mouseout', () => {
                    closeButton.classList.add('hidden'); // Hide when not hovered
                });

                modalItem.addEventListener('click', () => {
                    this.front(modal);
                    this.show(modalName); // Ensure the modal is shown
                    modalsList.classList.add('hidden');
                });

                modalItem.appendChild(modalText);
                modalItem.appendChild(closeButton);
                modalsList.appendChild(modalItem);
            });
            modalsList.classList.remove('hidden');
        } else {
            modalsList.classList.add('hidden');
        }
        document.addEventListener('click', this.hideModalsList);
    },

    hideModalsList: function(event) {
        const modalsList = document.getElementById('modals_list');
        if (!modalsList.contains(event.target) && event.target.id !== 'show_modals_button') {
            modalsList.classList.add('hidden');
            document.removeEventListener('click', modal.hideModalsList);
        }
    },

    show: function(modalId) {
        var modal = document.querySelector("[data-window='" + modalId + "']");
        if (modal) {
            modal.style.display = 'block';
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
            audio.playAudio("closModal", assets.load('closeModal'), 'sfx');
            this.modals = this.modals.filter(modal => modal.getAttribute('data-window') !== id);
            ui.unmount(id);

            if (window.modalResolves && window.modalResolves[id]) {
                console.log("resolving and removing", window.modalResolves[id]);
                window.modalResolves[id]();
                delete window.modalResolves[id];
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
