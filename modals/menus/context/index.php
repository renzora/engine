<div data-window="context_menu_window" data-close="false">

<div id="context_menu_window" class="bg-black opacity-70 text-white rounded-lg shadow-lg absolute z-50 hidden" style="max-height: 400px; min-width: 200px;">
        <ul id="menuItems" class="space-y-1"></ul>
    </div>

    <script>
        var context_menu_window = {
            menuItemsConfig: [{
                label: "Option 1", callback: () => editor_context_menu_window.advancedOption1()
            }],

            start: function() {
    this.contextMenuElement = document.getElementById('context_menu_window');
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


    disableDefaultContextMenu: function(event) {    
        event.preventDefault();
        context_menu_window.showContextMenu(event.clientX, event.clientY);
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

        context_menu_window.contextMenuElement.classList.add('hidden');
    }

        };

        context_menu_window.start();
    </script>
</div>
