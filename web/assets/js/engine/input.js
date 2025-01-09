input = {
    keys: {
        'ArrowUp': "up",
        'ArrowLeft': "left",
        'ArrowRight': "right",
        'ArrowDown': "down",
        'w': "up",
        'a': "left",
        's': "down",
        'd': "right"
    },
    eventListeners: {},
    isSpacePressed: false,
    isShiftPressed: false,
    isCtrlPressed: false,
    isAltPressed: false,
    isDragging: false,

    init: function() {
        this.assign("keydown", (e) => this.keyDown(e));
        this.assign("keyup", (e) => this.keyUp(e));
        this.assign('mousedown', (e) => this.mouseDown(e));
        this.assign('mousemove', (e) => this.mouseMove(e));
        this.assign('mouseup', (e) => this.mouseUp(e));
        this.assign('wheel', (e) => this.mouseWheelScroll(e), { passive: false });
        this.assign('click', (e) => this.leftClick(e));
        this.assign('dblclick', (e) => this.doubleClick(e));
        this.assign('contextmenu', (e) => this.rightClick(e));
        this.assign('resize', (e) => game.resizeCanvas(e));
    },

    assign: function(type, callback, options = {}) {
        if (!this.eventListeners[type]) {
            this.eventListeners[type] = new Set();
        }
        if (!this.eventListeners[type].has(callback)) {
            this.eventListeners[type].add(callback);
            window.addEventListener(type, callback, options);
        }
    },

    remove: function(type, callback) {
        if (this.eventListeners[type] && this.eventListeners[type].has(callback)) {
            this.eventListeners[type].delete(callback);
            window.removeEventListener(type, callback);
        }
    },

    removeAll: function(type) {
        if (type) {
            if (this.eventListeners[type]) {
                for (const callback of this.eventListeners[type]) {
                    window.removeEventListener(type, callback);
                }
                delete this.eventListeners[type];
            }
        } else {
            for (const [eventType, callbacks] of Object.entries(this.eventListeners)) {
                for (const callback of callbacks) {
                    document.removeEventListener(eventType, callback);
                }
            }
            this.eventListeners = {};
        }
    },

    updateInputMethod: function(method, name = '') {
        const inputMethodDisplay = document.getElementById('input_method');
        if (inputMethodDisplay) {
            inputMethodDisplay.innerText = `Input: ${method}${name ? ' (' + name + ')' : ''}`;
        }
    },

    keyDown: function(e) {
        if (game.isEditMode || this.isFormInputFocused()) return;
        if (!game.allowControls) return;
        this.updateInputMethod('keyboard');
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            if (e.key === 'Tab') {
                e.preventDefault();
            }
            if (e.key === ' ') {
                e.preventDefault();
            }
    
            if (e.key.toLowerCase() === 'x') {
                if (ui_overlay_window.remainingRounds > 0 && !ui_overlay_window.reloadInterval) {
                    ui_overlay_window.startReloading();
                } else if (ui_overlay_window.remainingBullets === 0 && ui_overlay_window.remainingRounds > 0) {
                    ui_overlay_window.handleReload();
                } else {
                    console.log("No rounds left to reload");
                    audio.playAudio("empty_gun", assets.use('empty_gun'), 'sfx', false);
                }
            }
    
            this.handleKeyDown(e);
        }
    },
    
    keyUp: function(e) {
        if (game.isEditMode || this.isFormInputFocused()) return;
        this.updateInputMethod('keyboard');
        if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
            e.preventDefault();
            this.handleKeyUp(e);
        }
    
        if (e.key.toLowerCase() === 'x') {
            ui_overlay_window.stopReloading();
        }
    },

    handleKeyDown: function(e) {
        this.handleControlStateChange(e, true);
    
        if (e.key === 'Tab') {
            e.preventDefault();
            plugin.load({
                id: 'console_window',
                url: 'editor/console/index.php',
                name: 'console',
                drag: false,
                reload: true,
                after: function () {
                    plugin.load({ id: 'edit_mode_window', url: 'editor/index.php', name: 'Editor', drag: true, reload: true });
                }
            });

        } else {
            const dir = this.keys[e.key];
            if (dir) {
                this.directions[dir] = true;
                this.updateSpriteDirections();
            }
        }
    
        if (e.key === 'f') {
            if (game.mainSprite) {
                game.mainSprite.targetAim = !game.mainSprite.targetAim;
                if (game.mainSprite.targetAim) {
                    console.log('Target aim activated');
                } else {
                    console.log('Target aim deactivated');
                }
            } else {
                console.error('Main sprite not found.');
            }
        } else if (e.key === ' ') {
            utils.fullScreen();
        }
    },

    handleKeyUp: function(e) {
        this.handleControlStateChange(e, false);

        if (e.keyCode === 27) {
            let maxZIndex = -Infinity;
            let maxZIndexElement = null;
            let attributeName = null;

            document.querySelectorAll("*").forEach(function (element) {
                const zIndex = parseInt(window.getComputedStyle(element).zIndex);
                if (!isNaN(zIndex) && zIndex > maxZIndex) {
                    maxZIndex = zIndex;
                    maxZIndexElement = element;
                    attributeName = element.getAttribute('data-attribute-name');
                }
            });

            if (maxZIndexElement) {
                maxZIndexElement.dispatchEvent(new Event('click'));
            } else if (attributeName) {
                const attributeElement = document.querySelector(`[data-attribute-name="${attributeName}"]`);
                if (attributeElement) {
                    attributeElement.dispatchEvent(new Event('click'));
                }
            }
        }

        const dir = this.keys[e.key];
        if (dir) {
            this.directions[dir] = false;
            this.updateSpriteDirections();
        }
    },

    mouseDown: function(e) {
        if (game.isEditMode) return;
        if (e.button === 1) {
            this.isDragging = true;
            this.startX = e.clientX;
            this.startY = e.clientY;
            document.body.classList.add('move-cursor');
        }
    
        if (e.button === 2) {
            this.cancelPathfinding(game.mainSprite);
        }
    },
    
    mouseMove: function(e) {
        if (!game.mainSprite) return;
        if (game.isEditMode) return;
        if (this.isDragging) {
            const dx = (this.startX - e.clientX) / game.zoomLevel;
            const dy = (this.startY - e.clientY) / game.zoomLevel;
    
            camera.cameraX = Math.max(0, Math.min(game.worldWidth - window.innerWidth / game.zoomLevel, camera.cameraX + dx));
            camera.cameraY = Math.max(0, Math.min(game.worldHeight - window.innerHeight / game.zoomLevel, camera.cameraY + dy));
    
            this.startX = e.clientX;
            this.startY = e.clientY;
        }
    
        if (game.mainSprite && game.mainSprite.targetAim) {
            const rect = game.canvas.getBoundingClientRect();
            const newX = (e.clientX - rect.left) / game.zoomLevel + camera.cameraX;
            const newY = (e.clientY - rect.top) / game.zoomLevel + camera.cameraY;
    
            game.mainSprite.targetX = newX;
            game.mainSprite.targetY = newY;
        }
    },
    
    mouseUp: function(e) {
        if (game.isEditMode) return;
        this.isDragging = false;
        document.body.classList.remove('move-cursor');
    },

    mouseWheelScroll: function(e) {

    },    

    leftClick: function(e) {
        if (game.isEditMode) return;
        this.updateInputMethod('keyboard');
        console.log("left button clicked");
        if (e.target.matches('[data-close], [data-esc]')) {
            console.log("data close clicked");
            var parent = plugin.closest(e.target);
            plugin.close(parent);
        }
    },
    
    rightClick: function(e) {
        if (game.isEditMode) return;
        e.preventDefault();
        this.updateInputMethod('keyboard');
        console.log("right button clicked");
        this.cancelPathfinding(game.mainSprite);
    },

    doubleClick: function(e) {},

    cancelPathfinding: function(sprite) {
        if (sprite && sprite.isMovingToTarget) {
            sprite.isMovingToTarget = false;
            sprite.path = [];
            sprite.moving = false;
            audio.stopLoopingAudio('footsteps1', 'sfx', 0.5);
        }
    },

    handleControlStateChange: function(e, isPressed) {
        switch (e.key) {
            case 'Shift':
                this.isShiftPressed = isPressed;
                break;
            case 'Control':
                this.isCtrlPressed = isPressed;
                break;
            case 'Alt':
                this.isAltPressed = isPressed;
                break;
            case ' ':
                this.isSpacePressed = isPressed;
                break;
        }
    },

    isFormInputFocused: function() {
        const activeElement = document.activeElement;
        return (
            activeElement &&
            (
                activeElement.tagName === 'INPUT' ||
                activeElement.tagName === 'TEXTAREA' ||
                activeElement.tagName === 'SELECT' ||
                activeElement.isContentEditable
            )
        );
    }    
};
