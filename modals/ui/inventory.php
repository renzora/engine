<div data-window='ui_inventory_window' data-close="false">

<div id="ui_inventory_window" class="fixed bottom-4 left-1/2 transform -translate-x-1/2 z-10 flex space-x-2 tracking-tight bg-[#0a0d14] rounded-md shadow-inner hover:shadow-lg p-1 border border-black">
    <div class="ui_item_primary relative flex items-center justify-center w-20 h-18 bg-[#18202f] rounded-md shadow-2xl hover:shadow-2xl transition-shadow duration-300"></div>
    <div class="flex flex-col space-y-2 w-98">
        <div class="flex items-center space-x-2 w-full">
            <div class="relative w-1/2 bg-gray-900 rounded-md h-6 overflow-hidden shadow-inner bg-opacity-80 shadow-sm p-[1px] flex items-center">
                <div class="mx-1">
                    <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
                    <div class="items_icon items_health scale-[1.2]"></div>
                </div>
                <div id="ui_health" class="rounded bg-gradient-to-r from-lime-500 to-green-600 h-full transition-width duration-500 flex-grow"></div>
                <div class="absolute inset-0 flex items-center pl-8 text-white text-sm">0%</div>
            </div>
            <div class="relative w-1/2 bg-gray-900 rounded-md h-6 overflow-hidden shadow-inner bg-opacity-80 shadow-sm p-[1px] flex items-center">
                <div class="mx-1">
                    <div class="items_icon items_energy scale-[1.2]"></div>
                </div>
                <div id="ui_energy" class="rounded bg-gradient-to-r from-cyan-400 to-blue-600 h-full transition-width duration-500 flex-grow"></div>
                <div class="absolute inset-0 flex items-center pl-8 text-white text-sm">0%</div>
            </div>
        </div>
        <div class="flex space-x-2" id="ui_quick_items_container"></div>
    </div>
</div>

<script>
var ui_inventory_window = {
    primaryItem: "sword",
    inventoryItems: [
        "potion",
        "shield",
        "banana",
        "skull",
        "wood",
        "black_emerald",
        "apple"
    ],
    currentItemIndex: 0,
    lastButtonPress: 0,
    throttleDuration: 150, // in milliseconds
    isItemSelected: false,

    start: function() {
        this.renderInventoryItems();
        this.initializeDragAndDrop();
        this.initializeQuickItems();
        this.initializePrimaryItem();
        this.displayInventoryItems();
        this.setupGamepadEvents();

        if (game.itemsData && game.itemsImg) {
            this.displayInventoryItems();
        } else {
            console.error("itemsData or itemsImg is not loaded.");
        }

        this.checkAndUpdateUIPositions();

        document.addEventListener('dragover', ui_inventory_window.documentDragOverHandler);
        document.addEventListener('drop', ui_inventory_window.documentDropHandler);
    },

    setupGamepadEvents: function() {
        window.addEventListener('gamepadConnected', () => {
            this.switchToGamepadMode();
        });
        window.addEventListener('gamepadDisconnected', () => {
            this.switchToKeyboardMode();
        });
    },

    l1Button: function(e) {
        if(!gamepad.buttons.includes('l2')) {
            this.handleGamepadInput(e, 'left');
        }
    },

    l2Button: function(e) {
        console.log("Left trigger called fron ui_inventory_window");
    },

    r1Button: function(e) {
        if(!gamepad.buttons.includes('l2')) {
            this.handleGamepadInput(e, 'right');
        }
    },

    aButton: function(e) {
        if (this.isItemSelected) {
            return; // Do nothing if an item is already selected
        }
        this.selectItem(this.currentItemIndex);
        this.highlightSelectedItem();
        this.isItemSelected = true; // Set the flag to true after selecting an item
    },

    switchToGamepadMode: function() {
        this.selectItem(0); // Select the primary item initially
    },

    switchToKeyboardMode: function() {
        this.clearHighlights();
    },

    handleGamepadInput: function(event, direction) {
        const currentTime = Date.now();
        if (currentTime - this.lastButtonPress < this.throttleDuration) {
            return; // Ignore if throttling
        }
        this.lastButtonPress = currentTime;

        if (direction === 'left') {
            this.currentItemIndex = (this.currentItemIndex - 1 + (this.inventoryItems.length + 1)) % (this.inventoryItems.length + 1);
        } else if (direction === 'right') {
            this.currentItemIndex = (this.currentItemIndex + 1) % (this.inventoryItems.length + 1);
        }

        console.log(`Current item index: ${this.currentItemIndex}`);
        audio.playAudio("menuSelect", assets.load('menuSelect'), 'sfx', false);
        this.selectItem(this.currentItemIndex);
    },

    highlightSelectedItem: function() {
        this.clearHighlights();
        let selectedItem;
        if (this.currentItemIndex === 0) {
            selectedItem = document.querySelector('.ui_item_primary');
        } else {
            selectedItem = document.querySelector(`.ui_quick_item[data-item="${this.inventoryItems[this.currentItemIndex - 1]}"]`);
        }

        if (selectedItem) {
            selectedItem.style.backgroundColor = 'blue'; // Highlight selected item with blue color
        }
        audio.playAudio("sceneDrop", assets.load('sceneDrop'), 'sfx', false);
    },

    selectItem: function(index) {
        this.clearHighlights();

        if (index === 0) {
            // Highlight the primary item
            const primaryItemElement = document.querySelector('.ui_item_primary');
            if (primaryItemElement) {
                primaryItemElement.classList.add('border-2', 'border-dashed', 'border-yellow-500');
            }
            this.primaryItem = "sword";
        } else {
            const itemElement = document.querySelector(`.ui_quick_item[data-item="${this.inventoryItems[index - 1]}"]`);
            if (itemElement) {
                itemElement.classList.add('border-2', 'border-dashed', 'border-yellow-500');
                this.primaryItem = this.inventoryItems[index - 1];
            }
        }

        // Update scale for all items after selection
        document.querySelectorAll('.ui_quick_item, .ui_item_primary').forEach(item => {
            this.updateScale(item);
        });

        audio.playAudio("menuDrop", assets.load('menuDrop'), 'sfx', false);
    },

    getCurrentPrimaryItemIndex: function() {
        return this.inventoryItems.indexOf(this.primaryItem) + 1;
    },

    renderInventoryItems: function() {
        const primaryItemElement = document.querySelector('.ui_item_primary');
        primaryItemElement.dataset.item = this.primaryItem;
        primaryItemElement.innerHTML = `
            <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
            <div class="items_icon items_${this.primaryItem} scale-[4] z-10"></div>
        `;

        const quickItemsContainer = document.getElementById('ui_quick_items_container');

        this.inventoryItems.forEach(itemName => {
            const itemElement = document.createElement('div');
            itemElement.className = 'ui_quick_item relative cursor-move w-12 h-12 bg-[#18202f] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center';
            itemElement.dataset.item = itemName;
            itemElement.innerHTML = `
                <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
                <div class="items_icon items_${itemName} scale-[2.1] z-10"></div>
            `;
            quickItemsContainer.appendChild(itemElement);
        });
    },

    unmount: function() {
        document.removeEventListener('dragover', this.documentDragOverHandler);
        document.removeEventListener('drop', this.documentDropHandler);
        this.dragClone = null;
    },

    documentDragOverHandler: function(e) {
        e.preventDefault();
        if (ui_inventory_window.dragClone) {
            ui_inventory_window.dragClone.style.top = `${e.clientY}px`;
            ui_inventory_window.dragClone.style.left = `${e.clientX}px`;
        }
    },

    documentDropHandler: function(e) {
        e.preventDefault();
        e.stopPropagation();

        if (ui_inventory_window.dragClone) {
            const rect = game.canvas.getBoundingClientRect();
            const mouseX = (e.clientX - rect.left) / game.zoomLevel + camera.cameraX;
            const mouseY = (e.clientY - rect.top) / game.zoomLevel + camera.cameraY;

            if (e.clientX >= rect.left && e.clientX <= rect.right && e.clientY <= rect.bottom) {
                const targetObject = game.findObjectAt(mouseX, mouseY);

                if (targetObject) {
                    const draggedItemIcon = ui_inventory_window.dragClone.querySelector('.items_icon');

                    if (draggedItemIcon) {
                        const itemClass = Array.from(draggedItemIcon.classList).find(cls => cls.startsWith('items_') && cls !== 'items_icon');

                        if (itemClass) {
                            const itemName = itemClass.replace('items_', '');
                            actions.dropItemOnObject(itemName, targetObject);
                        } else {
                            console.error('No specific item class found on dragged item icon');
                        }
                    } else {
                        console.error('Dragged item icon not found');
                    }
                }
            }

            document.body.removeChild(ui_inventory_window.dragClone);
            ui_inventory_window.dragClone = null;
        }

        return false;
    },

    displayInventoryItems: function() {
        if (!game.itemsData || !game.itemsData.items) {
            console.error("itemsData or items array is not defined.");
            return;
        }

        const inventoryItems = [this.primaryItem, ...this.inventoryItems];

        inventoryItems.forEach((itemName, index) => {
            const itemData = game.itemsData.items.find(item => item.name === itemName);
            if (itemData) {
                let itemElement;
                if (index === 0) {
                    itemElement = document.querySelector('.ui_item_primary');
                } else {
                    itemElement = document.querySelector(`.ui_quick_item[data-item="${itemName}"]`);
                }

                if (itemElement) {
                    this.setItemIcon(itemElement, itemData);
                    itemElement.dataset.cd = itemData.cd;
                    itemElement.querySelector('.items_icon').classList.add(`items_${itemName}`);
                }
            }
        });
    },

    setItemIcon: function(element, itemData) {
        const iconDiv = element.querySelector('.items_icon');
        if (iconDiv) {
            const iconSize = 16;
            const canvas = document.createElement('canvas');
            const ctx = canvas.getContext('2d');

            canvas.width = iconSize;
            canvas.height = iconSize;

            ctx.drawImage(
                game.itemsImg, 
                itemData.x, itemData.y, 
                iconSize, iconSize, 
                0, 0, 
                iconSize, iconSize
            );

            const dataURL = canvas.toDataURL();
            iconDiv.style.backgroundImage = `url(${dataURL})`;
            iconDiv.style.width = `${iconSize}px`;
            iconDiv.style.height = `${iconSize}px`;
            iconDiv.style.backgroundSize = 'cover';
        }
    },

    checkAndUpdateUIPositions: function() {
    const sprite = game.sprites[game.playerid];
    if (!sprite) return;

    const thresholdY = game.worldHeight - 50;
    const thresholdX = game.worldWidth - 80;

    const inventoryElement = document.getElementById('ui_inventory_window');
    if (inventoryElement) {
        if (sprite.y > thresholdY) {
            inventoryElement.classList.add('top-4');
            inventoryElement.classList.remove('bottom-4');
        } else {
            inventoryElement.classList.add('bottom-4');
            inventoryElement.classList.remove('top-4');
        }
    } else {
        console.error('Inventory element not found.');
    }

    const objectivesElement = document.getElementById('ui_objectives_window');
    if (objectivesElement) {
        if (sprite.x > thresholdX) {
            objectivesElement.classList.add('left-2');
            objectivesElement.classList.remove('right-2');
        } else {
            objectivesElement.classList.add('right-2');
            objectivesElement.classList.remove('left-2');
        }
    } else {
        console.error('Objectives element not found.');
    }
},

    initializeDragAndDrop: function() {
        const draggableItems = document.querySelectorAll('.ui_quick_item, .ui_item_primary');

        draggableItems.forEach(item => {
            item.setAttribute('draggable', true);
            item.style.cursor = 'grab';
            item.addEventListener('mouseover', this.handleMouseOver.bind(this));
            item.addEventListener('mouseout', this.handleMouseOut.bind(this));
            item.addEventListener('dragstart', this.handleDragStart.bind(this));
            item.addEventListener('dragover', this.handleDragOver.bind(this));
            item.addEventListener('drop', this.handleDrop.bind(this));
            item.addEventListener('dragend', this.handleDragEnd.bind(this));
        });
    },

    initializeQuickItems: function() {
        const quickItems = document.querySelectorAll('.ui_quick_item');
        quickItems.forEach(item => {
            item.addEventListener('click', () => {
                const cooldown = parseInt(item.dataset.cd, 10) * 1000;
                if (cooldown > 0) {
                    this.startTimeout(item, cooldown);
                }
            });
        });
    },

    initializePrimaryItem: function() {
        const primaryItem = document.querySelector('.ui_item_primary');
        primaryItem.addEventListener('click', () => {
            const cooldown = parseInt(primaryItem.dataset.cd, 10) * 1000;
            if (cooldown > 0) {
                this.startTimeout(primaryItem, cooldown);
            }
        });
    },

    startTimeout: function(item, duration) {
        if (!item.classList.contains('pointer-events-none')) {
            item.classList.add('pointer-events-none', 'opacity-80');
            const indicator = item.querySelector('.timeout-indicator');
            indicator.classList.remove('hidden');
            indicator.style.width = '100%';
            indicator.style.transitionDuration = `${duration}ms`;

            setTimeout(() => {
                indicator.style.width = '0%';
            }, 10);

            setTimeout(() => {
                item.classList.remove('pointer-events-none', 'opacity-80');
                indicator.style.transitionDuration = '0ms';
                indicator.style.width = '100%';
                indicator.classList.add('hidden');
            }, duration);
        }
    },

    activateTimeout: function(itemName, duration) {
        const item = document.querySelector(`[data-item="${itemName}"]`);
        if (item) {
            this.startTimeout(item, duration);
        } else {
            console.error(`Item with data-item "${itemName}" not found`);
        }
    },

    handleMouseOver: function(e) {
        e.target.style.cursor = 'grab';
    },

    handleMouseOut: function(e) {
        e.target.style.cursor = 'default';
    },

    handleDragStart: function(e) {
        this.dragSrcEl = e.target.closest('.ui_quick_item, .ui_item_primary');
        e.dataTransfer.effectAllowed = 'move';

        const iconDiv = this.dragSrcEl.querySelector('.items_icon');
        if (iconDiv) {
            const clonedIcon = iconDiv.cloneNode(true);
            const dragWrapper = document.createElement('div');
            dragWrapper.style.position = 'absolute';
            dragWrapper.style.top = `${e.clientY}px`;
            dragWrapper.style.left = `${e.clientX}px`;
            dragWrapper.style.pointerEvents = 'none';
            dragWrapper.style.zIndex = '1000';
            clonedIcon.style.transform = 'scale(4)';

            dragWrapper.appendChild(clonedIcon);
            this.dragClone = dragWrapper;
            document.body.appendChild(dragWrapper);
        }

        // Show grabbing cursor
        e.target.style.cursor = 'grabbing';

        var img = new Image();
        img.src = '';
        e.dataTransfer.setDragImage(img, 0, 0);
    },

    handleDragOver: function(e) {
        if (e.preventDefault) {
            e.preventDefault();
        }
        e.dataTransfer.dropEffect = 'move';

        if (this.dragClone) {
            this.dragClone.style.top = `${e.clientY}px`;
            this.dragClone.style.left = `${e.clientX}px`;
        }

        const target = e.target.closest('.ui_quick_item, .ui_item_primary');
        if (target) {
            this.clearHighlights();
            target.classList.add('border-2', 'border-dashed', 'border-yellow-500');
            if (!this.hasPlayedDragOverSound || this.lastHoveredSlot !== target) {
                audio.playAudio("menuDrop", assets.load('menuDrop'), 'sfx', false);
                this.hasPlayedDragOverSound = true;
                this.lastHoveredSlot = target;
            }
        } else {
            this.clearHighlights();
            this.hasPlayedDragOverSound = false;
            this.lastHoveredSlot = null;
        }
        return false;
    },

    handleDrop: function(e) {
        if (e.stopPropagation) {
            e.stopPropagation();
        }
        const target = e.target.closest('.ui_quick_item, .ui_item_primary');
        if (this.dragSrcEl !== target && target) {
            // Swap innerHTML to visually switch the items
            const tempInnerHTML = this.dragSrcEl.innerHTML;
            this.dragSrcEl.innerHTML = target.innerHTML;
            target.innerHTML = tempInnerHTML;

            // Swap dataset item to reflect the changes
            const tempDataItem = this.dragSrcEl.dataset.item;
            this.dragSrcEl.dataset.item = target.dataset.item;
            target.dataset.item = tempDataItem;

            // Update the inventoryItems array
            const srcIndex = this.inventoryItems.indexOf(tempDataItem);
            const targetIndex = this.inventoryItems.indexOf(this.dragSrcEl.dataset.item);
            this.inventoryItems[srcIndex] = this.dragSrcEl.dataset.item;
            this.inventoryItems[targetIndex] = tempDataItem;

            // Update currentItemIndex
            this.currentItemIndex = this.getCurrentPrimaryItemIndex();

            // Update the scale of the icons
            this.updateScale(this.dragSrcEl);
            this.updateScale(target);

            audio.playAudio("sceneDrop", assets.load('sceneDrop'), 'sfx', false);
        } else {
            audio.playAudio("slotDrop", assets.load('slotDrop'), 'sfx', false);
        }
        this.clearHighlights();
        return false;
    },

    clearHighlights: function() {
    const draggableItems = document.querySelectorAll('.ui_quick_item, .ui_item_primary');
    draggableItems.forEach(item => {
        item.classList.remove('border-2', 'border-dashed', 'border-yellow-500');
        item.style.backgroundColor = ''; // Clear background color
    });
    this.isItemSelected = false; // Reset the flag when items are deselected
},

    updateScale: function(element) {
        const icon = element.querySelector('.items_icon');
        if (element.classList.contains('ui_item_primary')) {
            icon.classList.remove('scale-[2.1]');
            icon.classList.add('scale-[4]');
        } else {
            icon.classList.remove('scale-[4]');
            icon.classList.add('scale-[2.1]');
        }
    },

    handleDragEnd: function(e) {
        const draggableItems = document.querySelectorAll('.ui_quick_item, .ui_item_primary');
        draggableItems.forEach(item => {
            item.classList.remove('dragging');
            item.style.cursor = 'grab'; // Reset the cursor to grab
            item.classList.remove('highlight');
        });

        if (this.dragClone) {
            document.body.removeChild(this.dragClone);
            this.dragClone = null;
        }
    },
};

ui_inventory_window.start();

</script>

</div>