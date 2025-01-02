<div data-window="context_menu_window" data-close="false">
    <div id="context_menu_window" class="bg-black opacity-70 text-white rounded-lg shadow-lg absolute z-50 hidden" style="max-height: 400px; min-width: 200px;">
        <ul id="menuItems" class="space-y-1"></ul>
    </div>

    <script>
        var context_menu_window = {
            menuItemsConfig: [],

            start: function() {
                this.contextMenuElement = document.getElementById('context_menu_window');
                this.menuItemsElement = document.getElementById('menuItems');

                document.addEventListener('contextmenu', this.disableDefaultContextMenu.bind(this));
                document.addEventListener('click', this.hideMenus.bind(this));
            },

            disableDefaultContextMenu: function(event) {
                event.preventDefault();
                this.populateMenuItems(event.clientX, event.clientY);
                this.positionContextMenu(event.clientX, event.clientY);
            },

            populateMenuItems: function(clientX, clientY) {
                this.clearMenuItems();

                const { mouseX, mouseY, gridX, gridY } = this.getMouseCoordinates(clientX, clientY);
                const selectedObject = utils.findObjectAt(mouseX, mouseY);
                const isTileWalkable = collision.isTileWalkable(gridX, gridY);

                this.menuItemsConfig = [];

                if (isTileWalkable) {
                    this.menuItemsConfig.push({
                        label: 'Walk Here',
                        callback: () => this.walkHere(gridX, gridY)
                    });
                }

                if (selectedObject) {
                    const objectData = game.objectData[selectedObject.id];
                    if (objectData && objectData[0] && objectData[0].n) {
                        const tileName = objectData[0].n;
                        this.menuItemsConfig.push({
                            label: `Edit ${tileName}`,
                            callback: () => this.editTile(selectedObject)
                        });
                    }
                }

                this.renderMenuItems();
            },

            renderMenuItems: function() {
                this.menuItemsConfig.forEach(item => {
                    const menuItem = document.createElement('li');
                    menuItem.textContent = item.label;
                    menuItem.classList.add('px-4', 'py-2', 'cursor-pointer', 'hover:bg-gray-900');
                    menuItem.onclick = item.callback;

                    if (item.subMenu) {
                        menuItem.classList.add('relative', 'group');

                        const arrow = document.createElement('span');
                        arrow.textContent = '▶';
                        arrow.classList.add('ml-2', 'text-gray-400', 'group-hover:text-white');
                        menuItem.appendChild(arrow);

                        const subMenu = this.createSubMenu(item.subMenu);
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
                    }

                    this.menuItemsElement.appendChild(menuItem);
                });
            },

            createSubMenu: function(items) {
                const subMenu = document.createElement('ul');
                subMenu.classList.add('absolute', 'hidden', 'bg-black', 'rounded-lg', 'shadow-lg', 'z-50');
                subMenu.style.minWidth = '200px';

                items.forEach(subItem => {
                    const subMenuItem = document.createElement('li');
                    subMenuItem.textContent = subItem.label;
                    subMenuItem.classList.add('px-4', 'py-2', 'cursor-pointer', 'hover:bg-gray-700');
                    subMenuItem.onclick = subItem.callback;

                    if (subItem.subMenu) {
                        subMenuItem.classList.add('relative', 'group');

                        const arrow = document.createElement('span');
                        arrow.textContent = '▶';
                        arrow.classList.add('ml-2', 'text-gray-400', 'group-hover:text-white');
                        subMenuItem.appendChild(arrow);

                        const nestedSubMenu = this.createSubMenu(subItem.subMenu);
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
                    }

                    subMenu.appendChild(subMenuItem);
                });

                return subMenu;
            },

            hideMenus: function(event) {
                if (event && ['INPUT', 'SELECT', 'TEXTAREA'].includes(event.target.tagName)) return;
                this.contextMenuElement.classList.add('hidden');
            },

            clearMenuItems: function() {
                this.menuItemsElement.innerHTML = '';
            },

            getMouseCoordinates: function(clientX, clientY) {
                const rect = game.canvas.getBoundingClientRect();
                const mouseX = (clientX - rect.left) / game.zoomLevel + camera.cameraX;
                const mouseY = (clientY - rect.top) / game.zoomLevel + camera.cameraY;
                const gridX = Math.floor(mouseX / 16);
                const gridY = Math.floor(mouseY / 16);
                return { mouseX, mouseY, gridX, gridY };
            },

            positionContextMenu: function(clientX, clientY) {
                this.contextMenuElement.style.left = `${Math.min(clientX, window.innerWidth - this.contextMenuElement.offsetWidth)}px`;
                this.contextMenuElement.style.top = `${Math.min(clientY, window.innerHeight - this.contextMenuElement.offsetHeight)}px`;
                this.contextMenuElement.classList.remove('hidden');
            },

            walkHere: function(gridX, gridY) {
                game.mainSprite.walkToClickedTile(gridX, gridY);
                this.hideMenus();
            },

            editTile: function(selectedObject) {
                if (selectedObject) {
                    const uniqueId = selectedObject.id;
                    modal.load({ id: 'tileset_item_editor_window', url: `renadmin/tileset/items.php?id=${uniqueId}`, name: 'Item Editor', drag: true, reload: true });
                }
            }
        };

        context_menu_window.start();
    </script>
</div>
