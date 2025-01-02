<div data-window="editor_context_menu_window" data-close="false">

<div id="editor_context_menu_window" class="bg-black opacity-70 text-white rounded-lg shadow-lg absolute z-50 hidden" style="max-height: 400px; min-width: 200px;">
        <ul id="menuItems" class="space-y-1"></ul>
    </div>

    <script>
        var editor_context_menu_window = {
            isGridEnabled: true,
            isSnapEnabled: true,
            isNightfilterEnabled: false,
            menuItemsConfig: [{
                label: "Edit",
                    subMenu: [
                        { label: "Manual Camera", type: "checkbox", id: "snap_checkbox", initialValue: true, callback: (checked) => editor_context_menu_window.cameraSnapToggle(checked)},
                        { label: "Adjust camera position", callback: () => editor_context_menu_window.openTerrainEditor() }
                    ]
                }, {
                label: "Scene",
                    subMenu: [
                        { label: "Scene Properties", callback: (clientX, clientY) => editor_context_menu_window.openSceneProperties(clientX, clientY) },
                        { label: "Set Background", callback: (clientX, clientY) => editor_context_menu_window.openSceneProperties(clientX, clientY) },
                        { label: "Advanced Options",
                            subMenu: [
                                { label: "Option 1", callback: () => editor_context_menu_window.advancedOption1() },
                                { label: "Option 2",
                                    subMenu: [
                                        { label: "Nested Option 1", callback: () => editor_context_menu_window.nestedOption1() },
                                        { label: "Nested Option 2",
                                            subMenu: [ 
                                                { label: "Deep Nested Option 1", callback: () => editor_context_menu_window.deepNestedOption1() },
                                                { label: "Deep Nested Option 2", callback: () => editor_context_menu_window.deepNestedOption2() }
                                            ]
                                        }
                                    ]
                                }
                            ]
                        }
                    ]},
                {
                label: "Sprite",
                    subMenu: [
                        { label: "Set Starting Position", callback: (clientX, clientY) => editor_context_menu_window.spriteSetStartingPosition(clientX, clientY) },
                        { label: "Sprite Active", type: "checkbox", id: "sprite_active_checkbox", initialValue: false, callback: (checked) => editor_context_menu_window.spriteActiveToggle(checked) }
                    ]
                },{
                label: "Camera",
                    subMenu: [
                        { label: "Manual Camera", type: "checkbox", id: "manual_camera_checkbox", initialValue: true, callback: (checked) => editor_context_menu_window.cameraManualToggle(checked)}
                    ]
                },{
                label: "Lighting",
                    subMenu: [
                        { label: "Lighting Sources", callback: (clientX, clientY) => editor_context_menu_window.spriteSetStartingPosition(clientX, clientY) }
                    ]
                }, {
                label: "Effects",
                    subMenu: [
                        { label: "Console Toggle", type: "checkbox", id: "console_toggle_checkbox", initialValue: false, callback: (checked) => editor_context_menu_window.effectsConsoleToggle(checked) },
                        { label: "Brush Size",type: "number", id: "brush_amount", initialValue: 1, callback: (value) => editor_context_menu_window.effectsBrushSize(value)}
                    ]
                }, {
                label: "Tools",
                    subMenu: [
                        { label: "Terrain Editor", callback: () => editor_context_menu_window.openTerrainEditor() },
                        { label: "Tileset Manager", callback: () => editor_context_menu_window.openTilesetManager() }
                    ]
                }, {
                label: "Utils",
                    subMenu: [
                        { label: "Grid", type: "checkbox", id: "toggle_grid_checkbox", initialValue: true, callback: (checked) => editor_context_menu_window.utilsToggleGrid(checked) },
                        { label: "Night Filter", type: "checkbox", id: "toggle_nightfilter_checkbox", initialValue: true, callback: (checked) => editor_context_menu_window.utilsToggleNightFilter(checked)},
                        { label: "Adjust Day/Time", callback: () => editor_context_menu_window.openTerrainEditor() },
                    ]
                }, {
                label: "Scripting",
                    subMenu: [
                        { label: "Manual Camera", type: "checkbox", id: "snap_checkbox", initialValue: true, callback: (checked) => editor_context_menu_window.cameraSnapToggle(checked) },
                        { label: "Adjust camera position", callback: () => editor_context_menu_window.openTerrainEditor() }
                        
                    ]
                }
            ],

            start: function() {
    this.contextMenuElement = document.getElementById('editor_context_menu_window');
    this.menuItemsElement = document.getElementById('menuItems');

    // Event listeners without binding
    document.addEventListener('contextmenu', this.disableDefaultContextMenu);
    document.addEventListener('click', this.hideMenus);
},

unmount: function() {
    // Remove event listeners
    document.removeEventListener('contextmenu', this.disableDefaultContextMenu);
    document.removeEventListener('click', this.hideMenus);

    console.log("Context menu unmounted and event listeners removed.");
},



disableDefaultContextMenu: (event) => {
    event.preventDefault();
    editor_context_menu_window.showContextMenu(event.clientX, event.clientY);
},


    showContextMenu: function(clientX, clientY) {
            // Clear existing menu items
            this.menuItemsElement.innerHTML = '';

            // Helper function to recursively create submenus
            const createSubMenu = (items) => {
                const subMenu = document.createElement('ul');
                subMenu.classList.add('absolute', 'hidden', 'bg-black', 'rounded-lg', 'shadow-lg', 'z-50');
                subMenu.style.minWidth = '200px';

                items.forEach((subItem) => {
                    const subMenuItem = document.createElement('li');
                    subMenuItem.classList.add('px-4', 'py-2', 'cursor-pointer', 'hover:bg-gray-700');
                    subMenuItem.style.userSelect = 'none';

                    subMenuItem.addEventListener('click', (event) => event.stopPropagation());

                    if (subItem.type === 'checkbox') {
                        const checkbox = document.createElement('input');
                        checkbox.type = 'checkbox';
                        checkbox.id = subItem.id;
                        checkbox.checked = subItem.initialValue;
                        checkbox.style.pointerEvents = 'none';

                        subMenuItem.addEventListener('click', () => {
                            checkbox.checked = !checkbox.checked;
                            subItem.initialValue = checkbox.checked;
                            subItem.callback(checkbox.checked);
                        });

                        subMenuItem.appendChild(checkbox);
                        subMenuItem.appendChild(document.createTextNode(` ${subItem.label}`));
                    } else if (subItem.type === 'number') {
                        const numberInput = document.createElement('input');
                        numberInput.type = 'number';
                        numberInput.id = subItem.id;
                        numberInput.value = subItem.initialValue;
                        numberInput.classList.add('ml-2', 'w-16', 'text-black', 'px-1', 'py-1', 'border', 'border-gray-600');

                        numberInput.addEventListener('input', (event) => {
                            subItem.initialValue = Number(event.target.value);
                            subItem.callback(Number(event.target.value));
                        });

                        numberInput.addEventListener('click', (event) => event.stopPropagation());

                        subMenuItem.textContent = subItem.label;
                        subMenuItem.appendChild(numberInput);
                    } else if (subItem.subMenu) {
                        subMenuItem.textContent = subItem.label;

                        const arrow = document.createElement('span');
                        arrow.textContent = '▶';
                        arrow.classList.add('ml-2', 'text-gray-400', 'group-hover:text-white');
                        subMenuItem.appendChild(arrow);

                        subMenuItem.classList.add('relative', 'group');
                        const nestedSubMenu = createSubMenu(subItem.subMenu);
                        subMenuItem.appendChild(nestedSubMenu);

                        subMenuItem.addEventListener('mouseenter', () => {
                            nestedSubMenu.classList.remove('hidden');
                            const menuRect = subMenuItem.getBoundingClientRect();
                            nestedSubMenu.style.left = `${menuRect.width}px`;
                            nestedSubMenu.style.top = '0';
                        });

                        subMenuItem.addEventListener('mouseleave', () => {
                            nestedSubMenu.classList.add('hidden');
                        });
                    } else {
                        subMenuItem.textContent = subItem.label;
                        if (subItem.callback) {
                            subMenuItem.onclick = (e) => subItem.callback(e.clientX, e.clientY);
                        }
                    }

                    subMenu.appendChild(subMenuItem);
                });

                return subMenu;
            };

            // Build the top-level menu items
            this.menuItemsConfig.forEach((item) => {
                const menuItem = document.createElement('li');
                menuItem.classList.add('px-4', 'py-2', 'cursor-pointer', 'hover:bg-gray-900');

                if (item.subMenu) {
                    menuItem.textContent = item.label;

                    const arrow = document.createElement('span');
                    arrow.textContent = '▶';
                    arrow.classList.add('ml-2', 'text-gray-400', 'group-hover:text-white');
                    menuItem.appendChild(arrow);

                    menuItem.classList.add('relative', 'group');
                    const subMenu = createSubMenu(item.subMenu);
                    menuItem.appendChild(subMenu);

                    menuItem.addEventListener('mouseenter', () => {
                        subMenu.classList.remove('hidden');
                        const menuRect = menuItem.getBoundingClientRect();
                        subMenu.style.left = `${menuRect.width}px`;
                        subMenu.style.top = '0';
                    });

                    menuItem.addEventListener('mouseleave', () => {
                        subMenu.classList.add('hidden');
                    });
                } else {
                    menuItem.textContent = item.label;
                    if (item.callback) {
                        menuItem.onclick = (e) => item.callback(e.clientX, e.clientY);
                    }
                }

                this.menuItemsElement.appendChild(menuItem);
            });

            // Position and display the context menu
            this.contextMenuElement.style.left = `${Math.min(clientX, window.innerWidth - this.contextMenuElement.offsetWidth)}px`;
            this.contextMenuElement.style.top = `${Math.min(clientY, window.innerHeight - this.contextMenuElement.offsetHeight)}px`;
            this.contextMenuElement.classList.remove('hidden');
        },

    hideMenus: function(event) {
        if (event && ['INPUT', 'SELECT', 'TEXTAREA'].includes(event.target.tagName)) {
            return;
        }

        editor_context_menu_window.contextMenuElement.classList.add('hidden');
    },

            toggleGrid: function(checked) {
                this.isGridEnabled = checked;
                if (checked) {
                    console.log("Grid is now enabled.");
                    this.render2dGrid();
                } else {
                    console.log("Grid is now disabled.");
                }
            },

            toggleNightFilter: function(checked) {
                this.isNightfilterEnabled = checked;

                if (checked) {
                    lighting.nightFilterActive = true;
                    utils.gameTime.hours = 0;
                    console.log("Night filter enabled.");
                } else {
                    lighting.nightFilterActive = false;
                    lighting.timeBasedUpdatesEnabled = false;
                    utils.gameTime.hours = 12;
                    console.log("Night filter disabled.");
                }

            },

            render2dGrid: function () {
    if (!this.isGridEnabled) return;

    // Set grid line style to be dark and subtle
    game.ctx.strokeStyle = 'rgba(0, 0, 0, 0.1)';
    game.ctx.lineWidth = 1;

    // Draw vertical lines
    for (let x = 0; x < game.worldWidth; x += 16) {
        game.ctx.beginPath();
        game.ctx.moveTo(x, 0);
        game.ctx.lineTo(x, game.worldHeight);
        game.ctx.stroke();
    }

    // Draw horizontal lines
    for (let y = 0; y < game.worldHeight; y += 16) {
        game.ctx.beginPath();
        game.ctx.moveTo(0, y);
        game.ctx.lineTo(game.worldWidth, y);
        game.ctx.stroke();
    }

    // Draw a red border around the outer edge of the grid
    game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
    game.ctx.lineWidth = 2; // Make the border slightly thicker
    game.ctx.strokeRect(0, 0, game.worldWidth, game.worldHeight);
},

renderIsometricGrid: function () {
    if (!this.isGridEnabled) return;

    const tileWidth = 32;
    const tileHeight = 16;
    
    // Instead of halfWorldWidth / halfWorldHeight, use the full game.worldWidth / game.worldHeight
    // so we can overshoot and ensure the diagonal lines fill the entire canvas
    const maxY = game.worldHeight;
    const minY = -game.worldHeight;

    // Light grid lines
    game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
    game.ctx.lineWidth = 1;

    // Draw lines slanted to the right
    for (let y = minY; y <= maxY; y += tileHeight) {
        game.ctx.beginPath();
        // Move from left edge down
        game.ctx.moveTo(0, game.worldHeight / 2 + y);
        // Draw to right edge, shifted up by half the width
        game.ctx.lineTo(game.worldWidth, game.worldHeight / 2 + y - game.worldWidth / 2);
        game.ctx.stroke();
    }

    // Draw lines slanted to the left
    for (let y = minY; y <= maxY; y += tileHeight) {
        game.ctx.beginPath();
        // Move from right edge down
        game.ctx.moveTo(game.worldWidth, game.worldHeight / 2 + y);
        // Draw to left edge, shifted up by half the width
        game.ctx.lineTo(0, game.worldHeight / 2 + y - game.worldWidth / 2);
        game.ctx.stroke();
    }

    // Border (optional)
    game.ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
    game.ctx.lineWidth = 2;
    game.ctx.strokeRect(0, 0, game.worldWidth, game.worldHeight);
},


            updateStartingPosition: function(gridX, gridY) {
                const sceneId = game.sceneid;
                if (!sceneId) {
                    alert('No scene loaded!');
                    return;
                }

                fetch('/modals/editor/ajax/setSpritePosition.php', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({
                        sceneId: sceneId,
                        startingX: gridX,
                        startingY: gridY
                    }),
                })
                .then(response => response.json())
                .then(data => {
                    if (data.error) {
                        alert(`Error: ${data.message}`);
                    } else {
                        alert('Starting position updated successfully.');
                        game.roomData.startingX = gridX;
                        game.roomData.startingY = gridY;
                    }
                })
                .catch(error => {
                    console.error('Error updating starting position:', error);
                    alert('An error occurred. Check the console for details.');
                });
            },

            openSceneProperties: function(clientX, clientY) {
                modal.load({ id: 'editor_scene_properties_window', url: 'editor/modules/scene_properties.php', name: 'Scene Properties', drag: true, reload: true });
                this.hideMenus();
            },

            spriteSetStartingPosition: function(clientX, clientY) {
                const rect = game.canvas.getBoundingClientRect();
                const mouseX = (clientX - rect.left) / game.zoomLevel + camera.cameraX;
                const mouseY = (clientY - rect.top) / game.zoomLevel + camera.cameraY;
                const gridX = Math.floor(mouseX / 16);
                const gridY = Math.floor(mouseY / 16);

                this.updateStartingPosition(gridX, gridY);
                this.hideMenus();
            },
            spriteActiveToggle: function(checked) {
                if (checked) {
        camera.lerpEnabled = false;
        camera.manual = true;
        console.log("Sprite Enabled");
    } else {
        camera.lerpEnabled = true;
        camera.manual = false;
        console.log("Sprite Disabled");
    }
                
            },
            cameraSetStartingPosition: function(clientX, clientY) {
                const rect = game.canvas.getBoundingClientRect();
                const mouseX = (clientX - rect.left) / game.zoomLevel + camera.cameraX;
                const mouseY = (clientY - rect.top) / game.zoomLevel + camera.cameraY;
                const gridX = Math.floor(mouseX / 16);
                const gridY = Math.floor(mouseY / 16);

                this.updateStartingPosition(gridX, gridY);
                this.hideMenus();
            },
            cameraManualToggle: function(checked) {
    if (checked) {
        camera.lerpEnabled = false;
        camera.manual = true;
        console.log("Manual Camera Enabled");
    } else {
        camera.lerpEnabled = true;
        camera.manual = false;
        console.log("Manual Camera Disabled");
    }
},

            openTilesetManager: function() {
                modal.load({ id: 'tileset_window', url: 'renadmin/tileset/index.php', name: 'Tileset Manager', drag: true, reload: false });
            },
            utilsToggleGrid: function(checked) {
                console.log(`Grid toggled: ${checked}`);
                this.toggleGrid(checked);
            },
            utilsToggleNightFilter: function(checked) {
                this.toggleNightFilter(checked);
            },
            openHelpTutorials: function() {
                console.log("Open Help Tutorials");
                document.getElementById("editor_help_tutorials").click();
            }

        };

        editor_context_menu_window.start();
    </script>
</div>
