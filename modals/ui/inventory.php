<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>

<div data-window='ui_inventory_window' data-close="false" class="fixed bottom-8 left-1/2 transform -translate-x-1/2 z-10 flex flex-col items-start space-y-0">

  <!-- Inventory Slots -->
  <div id="ui_inventory_window" class="pixel-corners flex space-x-2 bg-[#0a0d14] p-2 shadow-inner hover:shadow-lg border border-black rounded-lg">
    <div class="flex space-x-2" id="ui_quick_items_container"></div>
  </div>

  <script>
var ui_inventory_window = {
    inventory: [
        { key: "66cdbbc45f5b8", amount: 0, damage: 60 },
    ],

    currentItemIndex: 0,
    lastButtonPress: 0,
    throttleDuration: 150,
    isItemSelected: false,
    targetItemIndex: null,
    dragClone: null,
    inTabSwitchingMode: false,
    currentTabButtonIndex: 0,
    isAButtonHeld: false,

    start: function() {
        this.renderInventoryItems();
        this.initializeDragAndDrop();
        this.initializeQuickItems();
        this.displayInventoryItems();
        this.setupGamepadEvents();

        if (game.itemsData && game.itemsImg) {
            this.displayInventoryItems();
        } else {
            console.error("itemsData or itemsImg is not loaded.");
        }

        this.selectItem(0);

        document.addEventListener('dragover', this.documentDragOverHandler.bind(this));
        document.addEventListener('drop', this.documentDropHandler.bind(this));
    },

    clearTabHighlights: function() {
        document.querySelectorAll('.tab-button').forEach(button => {
            button.classList.remove('bg-[#3c4e6f]', 'bg-yellow-500');
            button.classList.add('bg-[#0a0d14]');
        });
    },

    highlightSelectedTab: function() {
        // Ensure the currently active tab remains highlighted in grey
        const activeButton = document.querySelector(`.tab-button[data-tab="${this.currentTab}"]`);
        if (activeButton) {
            activeButton.classList.add('bg-[#3c4e6f]');
        }
    },

    switchTab: function(tabName) {
    this.currentTab = tabName;
    this.currentItemIndex = 0; // Reset to the first item
    this.renderInventoryItems();
    this.displayInventoryItems();

    // Safely select the first item or empty slot
    const firstItemIndex = this.getFilteredInventory().findIndex(item => item !== null);
    this.selectItem(firstItemIndex !== -1 ? firstItemIndex : 0);

    // Ensure all buttons are reset to their default color
    this.clearTabHighlights();

    // Highlight the selected tab with the grey color
    this.highlightSelectedTab();

    this.initializeDragAndDrop();
    this.initializeQuickItems();
},

setupGamepadEvents: function() {
        window.addEventListener('gamepadConnected', () => {
            this.switchToGamepadMode();
        });
        window.addEventListener('gamepadDisconnected', () => {
            this.switchToKeyboardMode();
        });

        gamepad.throttle((e) => this.leftButton(e), this.throttleDuration);
        gamepad.throttle((e) => this.rightButton(e), this.throttleDuration);
        gamepad.throttle((e) => this.aButton(e), this.throttleDuration);  // Mapping A button
        gamepad.throttle(() => this.bButton(), this.throttleDuration);

        gamepad.throttle((e) => this.upButton(e), this.throttleDuration);
        gamepad.throttle((e) => this.downButton(e), this.throttleDuration);
    },

    switchToGamepadMode: function() {
        console.log("Switched to gamepad mode.");
        this.selectItem(0);
    },

    throttle: function(callback) {
        const currentTime = Date.now();
        if (currentTime - this.lastButtonPress < this.throttleDuration) {
            return false;
        }
        this.lastButtonPress = currentTime;
        callback();
        return true;
    },

    leftButton: function(e) {
        this.throttle(() => {
            this.currentItemIndex = (this.currentItemIndex - 1 + 10) % 10;  // Move left through the slots, including empty ones
            this.selectItem(this.currentItemIndex);
            audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
        });
    },

    rightButton: function(e) {
        this.throttle(() => {
            this.currentItemIndex = (this.currentItemIndex + 1) % 10;  // Move right through the slots, including empty ones
            this.selectItem(this.currentItemIndex);
            audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
        });
    },

    upButton: function(e) {
        if (!this.inTabSwitchingMode) {
            this.inTabSwitchingMode = true;
            this.highlightTabButton(this.currentTabButtonIndex);
            audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
        }
    },

    downButton: function(e) {
        if (this.inTabSwitchingMode) {
            this.inTabSwitchingMode = false;
            this.clearTabHighlights();
            this.highlightSelectedTab();  // Ensure the active tab remains grey
            audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
        }
    },

    aButton: function(e) {
        this.throttle(() => {
            if (!this.isItemSelected) {
                // Enter swap mode and highlight the selected item with green, or keep the yellow border on an empty slot
                this.isItemSelected = true;
                this.highlightSelectedItem();  // Highlights the item or empty slot
                this.targetItemIndex = this.currentItemIndex;  // Set the target index for swapping
                audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            } else {
                // Attempt to swap the items, but still allow highlighting of an empty slot
                this.swapItems();
                this.isItemSelected = false;
                this.clearHighlights();
                this.selectItem(this.currentItemIndex); // Re-select the current item to maintain the yellow border
                audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            }
        });
    },

    bButton: function(e) {
        this.throttle(() => {
            if (this.isItemSelected) {
                // Exit swap mode if B is pressed
                this.isItemSelected = false;
                this.clearHighlights();  // Clear the green and yellow highlights
            } else {
                // Implement any other behavior for B button when not in swap mode
                console.log('B button pressed, no swap mode active.');
                audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            }
        });
    },

    enterTabButton: function(e) {
        if (this.inTabSwitchingMode) {
            const selectedTab = this.getTabButtons()[this.currentTabButtonIndex].getAttribute('data-tab');
            this.switchTab(selectedTab);
            this.inTabSwitchingMode = false;
            this.highlightSelectedTab();  // Ensure the new active tab is highlighted in grey
        }
    },

    highlightTabButton: function(index) {
        const tabButtons = this.getTabButtons();

        // Clear previous yellow highlights
        tabButtons.forEach((button, i) => {
            if (!button.classList.contains('bg-[#3c4e6f]')) {
                button.classList.remove('bg-yellow-500');
                button.classList.add('bg-[#0a0d14]');
            }
        });

        // Highlight the currently selecting tab with yellow
        tabButtons[index].classList.add('bg-yellow-500');
    },

    getTabButtons: function() {
        return Array.from(document.querySelectorAll('.tab-button'));
    },

    swapItems: function() {
    const selectedItemIndex = this.currentItemIndex;
    const targetIndex = this.targetItemIndex;

    console.log(`Before Swap:`);
    console.log(`Selected Item Index: ${selectedItemIndex}`);
    console.log(`Target Item Index: ${targetIndex}`);
    console.log('Filtered Inventory:', this.getFilteredInventory().map(item => item ? item.name : 'empty slot'));

    const selectedItem = this.getFilteredInventory()[selectedItemIndex];
    const targetItem = this.getFilteredInventory()[targetIndex];

    // Only proceed with the swap if both slots have items
    if (selectedItem && targetItem) {
        const selectedInventoryIndex = this.inventory.findIndex(item => item.name === selectedItem.name);
        const targetInventoryIndex = this.inventory.findIndex(item => item.name === targetItem.name);

        if (selectedInventoryIndex !== -1 && targetInventoryIndex !== -1) {
            // Swap items directly in the inventory array based on visual slot index
            [this.inventory[selectedInventoryIndex], this.inventory[targetInventoryIndex]] = 
            [this.inventory[targetInventoryIndex], this.inventory[selectedInventoryIndex]];

            // Maintain the currentItemIndex based on the visual slot
            this.currentItemIndex = targetIndex;
        }

        console.log(`After Swap:`);
        console.log(`Current Item Index: ${this.currentItemIndex}`);
        console.log('Filtered Inventory:', this.getFilteredInventory().map(item => item ? item.name : 'empty slot'));

        this.clearHighlights();
        this.isItemSelected = false;
        this.targetItemIndex = null;
    } else {
        console.error('Cannot swap items: one or both slots are empty.');
        // Clear highlights and exit swap mode without swapping
        this.clearHighlights();
        this.isItemSelected = false;
        this.targetItemIndex = null;
        // Ensure the current slot remains highlighted
        this.selectItem(this.currentItemIndex);
    }
},

    highlightTargetItem: function(direction) {
        this.clearHighlights(false);

        let newIndex;

        if (direction === 'left') {
            newIndex = (this.currentItemIndex - 1 + this.getFilteredInventory().length) % this.getFilteredInventory().length;
        } else if (direction === 'right') {
            newIndex = (this.currentItemIndex + 1) % this.getFilteredInventory().length;
        }

        this.targetItemIndex = newIndex;

        let targetItem = document.querySelector(`.ui_quick_item[data-item="${this.getFilteredInventory()[this.targetItemIndex].name}"]`);
        if (targetItem) {
            targetItem.classList.add('border-2', 'border-green-500');
        }
    },

    highlightSelectedItem: function() {
    this.clearHighlights();

    const selectedItem = this.getFilteredInventory()[this.currentItemIndex];

    if (selectedItem) {
        // Highlight the item if it exists
        const selectedItemElement = document.querySelector(`.ui_quick_item[data-item="${selectedItem.name}"]`);
        if (selectedItemElement) {
            selectedItemElement.classList.add('border-2', 'border-green-500');
        }
    } else {
        // Highlight the empty slot with a yellow border
        const itemElements = document.querySelectorAll('.ui_quick_item');
        const itemElement = itemElements[this.currentItemIndex];
        if (itemElement) {
            itemElement.classList.add('border-2', 'border-dashed', 'border-yellow-500');
        }
    }
},
selectItem: function(index) {
    this.clearHighlights();

    const filteredInventory = this.getFilteredInventory();
    const itemElements = document.querySelectorAll('.ui_quick_item');
    const itemElement = itemElements[index];

    if (itemElement) {
        itemElement.classList.add('border-2', 'border-dashed', 'border-yellow-500');
    } else {
        console.error('Item element not found for index:', index);
    }

    const selectedItem = filteredInventory[index];
    const sprite = game.sprites[game.playerid];
    if (selectedItem && sprite) {
        console.log('Selected Item:', selectedItem);
        sprite.currentItem = selectedItem.name;
    } else if (sprite) {
        console.log('Selected empty slot');
        sprite.currentItem = null;  // Set the currentItem to null for an empty slot
    }
},

    clearHighlights: function(clearBlueBackground = true) {
        const draggableItems = document.querySelectorAll('.ui_quick_item');
        draggableItems.forEach(item => {
            item.classList.remove('border-2', 'border-dashed', 'border-yellow-500', 'border-green-500');
            if (clearBlueBackground) {
                item.style.backgroundColor = '';
            }
        });
    },

    renderInventoryItems: function() {
    const quickItemsContainer = document.getElementById('ui_quick_items_container');
    quickItemsContainer.innerHTML = '';

    const filteredItems = this.getFilteredInventory();

    for (let i = 0; i < 10; i++) {
        const item = filteredItems[i];
        const itemElement = document.createElement('div');
        itemElement.className = 'ui_quick_item relative cursor-move w-14 h-14 bg-[#18202f] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center';
        itemElement.dataset.item = item ? item.key : '';

        if (item) {
            const objectData = game.objectData[item.key] ? game.objectData[item.key][0] : null;

            if (objectData) {
                const index = objectData.i[0].split('-');
                const tileIndex = parseInt(index[0], 10);
                const tileX = (tileIndex % 150) * 16;
                const tileY = Math.floor(tileIndex / 150) * 16;

                const canvas = document.createElement('canvas');
                canvas.width = 16;
                canvas.height = 16;
                const ctx = canvas.getContext('2d');
                const spriteSheet = assets.use(objectData.t);

                ctx.drawImage(spriteSheet, tileX, tileY, 16, 16, 0, 0, 16, 16);

                const imageDataUrl = canvas.toDataURL();
                const condition = 100 - (item.damage || 0);
                let barColor = 'bg-green-500';
                if (condition <= 15) barColor = 'bg-red-500';
                else if (condition <= 50) barColor = 'bg-orange-500';

                itemElement.innerHTML = `
                    <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
                    <img class="items_icon scale-[2.1] z-10" src="${imageDataUrl}" width="16" height="16" />
                    ${item.amount > 1 ? `
                    <div class="item-badge absolute top-0 left-0 z-20 bg-[#18202f] text-white rounded-full text-xs w-5 h-5 flex items-center justify-center">
                        ${item.amount}
                    </div>
                    ` : ''}
                    <div class="damage-bar absolute bottom-0 left-0 right-0 h-1 ${barColor} rounded-full" style="width: ${condition}%;"></div>
                `;
            }
        } else {
            itemElement.innerHTML = `
                <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
            `;
        }

        quickItemsContainer.appendChild(itemElement);
    }
},


getFilteredInventory: function() {
    // This ensures that if the inventory is empty, we still return an array with 15 empty slots
    const filtered = this.inventory.filter(item => item);
    const emptySlots = Array(10 - filtered.length).fill(null);  // Fill with `null` for empty slots
    return [...filtered, ...emptySlots];  // Merge filled items and empty slots
},

    unmount: function() {
        document.removeEventListener('dragover', this.documentDragOverHandler);
        document.removeEventListener('drop', this.documentDropHandler);
        this.dragClone = null;
    },

    documentDragOverHandler: function(e) {
        e.preventDefault();
        if (this.dragClone) {
            this.dragClone.style.top = `${e.clientY}px`;
            this.dragClone.style.left = `${e.clientX}px`;
        }
    },

    documentDropHandler: function(e) {
        e.preventDefault();
        e.stopPropagation();

        if (this.dragClone) {
            const rect = game.canvas.getBoundingClientRect();
            const mouseX = (e.clientX - rect.left) / game.zoomLevel + camera.cameraX;
            const mouseY = (e.clientY - rect.top) / game.zoomLevel + camera.cameraY;

            if (e.clientX >= rect.left && e.clientX <= rect.right && e.clientY <= rect.bottom) {
                const targetObject = utils.findObjectAt(mouseX, mouseY);

                if (targetObject) {
                    const draggedItemIcon = this.dragClone.querySelector('.items_icon');

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

            document.body.removeChild(this.dragClone);
            this.dragClone = null;
        }

        return false;
    },

    displayInventoryItems: function() {
        if (!game.objectData) {
            console.error("objectData is not defined.");
            return;
        }

        this.getFilteredInventory().forEach((item, index) => {
            if (!item) return; // Skip null or empty slots

            const objectData = game.objectData[item.key] ? game.objectData[item.key][0] : null;

            if (objectData) {
                let itemElement = document.querySelector(`.ui_quick_item[data-item="${item.key}"]`);
                if (itemElement) {
                    this.setItemIcon(itemElement, objectData);
                }
            } else {
                console.error('Object data not found for key:', item.key);
            }
        });
    },

    setItemIcon: function(element, objectData) {
        const iconDiv = element.querySelector('.items_icon');
        if (iconDiv) {
            const index = objectData.i[0].split('-'); // e.g., "472-472"
            const tileIndex = parseInt(index[0], 10);
            const tileX = (tileIndex % 150) * 16;
            const tileY = Math.floor(tileIndex / 150) * 16;

            const canvas = document.createElement('canvas');
            canvas.width = 16;
            canvas.height = 16;
            const ctx = canvas.getContext('2d');

            // Load the actual image from assets
            const spriteSheet = assets.use(objectData.t);

            // Draw the sprite on the canvas
            ctx.drawImage(spriteSheet, tileX, tileY, 16, 16, 0, 0, 16, 16);

            // Set the icon's src as the canvas data URL
            iconDiv.src = canvas.toDataURL();
            iconDiv.width = 16;
            iconDiv.height = 16;
        }
    },

    initializeDragAndDrop: function() {
        const draggableItems = document.querySelectorAll('.ui_quick_item');

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
    this.dragSrcEl = e.target.closest('.ui_quick_item');

    // Prevent dragging if the slot is empty
    if (!this.dragSrcEl || !this.dragSrcEl.dataset.item) {
        e.preventDefault();
        return;
    }

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

        const target = e.target.closest('.ui_quick_item');
        if (target) {
            this.clearHighlights();
            target.classList.add('border-2', 'border-dashed', 'border-yellow-500');
            if (!this.hasPlayedDragOverSound || this.lastHoveredSlot !== target) {
                audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
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
    const target = e.target.closest('.ui_quick_item');
    if (this.dragSrcEl !== target && target) {
        const tempInnerHTML = this.dragSrcEl.innerHTML;
        const tempDataItem = this.dragSrcEl.dataset.item;

        // Ensure both items exist before swapping
        if (tempDataItem && target.dataset.item) {
            this.dragSrcEl.innerHTML = target.innerHTML;
            target.innerHTML = tempInnerHTML;

            this.dragSrcEl.dataset.item = target.dataset.item;
            target.dataset.item = tempDataItem;

            const srcIndex = this.inventory.findIndex(item => item.key === tempDataItem);
            const targetIndex = this.inventory.findIndex(item => item.key === this.dragSrcEl.dataset.item);

            if (srcIndex !== -1 && targetIndex !== -1) {
                [this.inventory[srcIndex], this.inventory[targetIndex]] = [this.inventory[targetIndex], this.inventory[srcIndex]];

                if (this.currentItemIndex === srcIndex) {
                    this.currentItemIndex = targetIndex;
                }
            }
        } else if (tempDataItem) {
            // If target is empty, move the item to the target slot
            target.innerHTML = tempInnerHTML;
            target.dataset.item = tempDataItem;

            this.dragSrcEl.innerHTML = '';
            this.dragSrcEl.dataset.item = '';

            const srcIndex = this.inventory.findIndex(item => item.key === tempDataItem);
            if (srcIndex !== -1) {
                this.inventory.splice(srcIndex, 1);  // Remove the item from the original position
                this.inventory.push({ key: tempDataItem, amount: 1, damage: 0 }); // Add it to the end
                this.currentItemIndex = this.inventory.length - 1;
            }
        }

        this.updateScale(this.dragSrcEl);
        this.updateScale(target);

        this.updateItemBadges();

        this.clearHighlights();
        this.selectItem(this.currentItemIndex);

        audio.playAudio("sceneDrop", assets.use('sceneDrop'), 'sfx', false);
    } else {
        audio.playAudio("slotDrop", assets.use('slotDrop'), 'sfx', false);
    }
    return false;
},

addToInventory: function(itemKey) {
    // Ensure that the inventory array is initialized
    if (!Array.isArray(this.inventory)) {
        console.error("Inventory array is not initialized.");
        return;
    }

    // Check if the item already exists in the inventory
    const existingItem = this.inventory.find(item => item.key === itemKey);

    // Check if the inventory is full (assumed maximum capacity is 10)
    const filteredItems = this.getFilteredInventory().filter(item => item !== null);
    if (filteredItems.length >= 10 && !existingItem) {
        console.log("Inventory is full. Cannot add new items.");
        return; // Exit if the inventory is full and the item does not already exist
    }

    // Look up the item in the objectData using the item key
    const itemData = game.objectData[itemKey] && game.objectData[itemKey][0]; // Access the first element of the array

    if (!itemData) {
        console.error(`Item data for ${itemKey} not found.`);
        return;
    }

    // If the item is already in the inventory and has collect set to false, prevent adding it again
    if (existingItem && itemData.collect === false) {
        console.log(`${itemKey} is already in the inventory and cannot be collected again.`);
        return;
    }

    // Track the currently selected index before adding the new item
    const previousSelectedIndex = this.currentItemIndex;

    // Remove the item from roomData if it's being collected for the first time or if it can be collected multiple times
    if (!existingItem || itemData.collect !== false) {
        const itemIndex = game.roomData.items.findIndex(item => item.id === itemKey);
        if (itemIndex !== -1) {
            game.roomData.items.splice(itemIndex, 1); // Remove the item from roomData
        }
    }

    // Set the collected flag to true for this item if it has collect set to false
    if (itemData.collect === false) {
        itemData.collected = true;
    }

    // Check if the item is already in the current tab
    const tabItem = this.inventory.find(item => item.key === itemKey);

    if (tabItem) {
        tabItem.amount += 1; // Increase the amount if it already exists in the inventory
    } else {
        // Add new item to the inventory if not already present
        this.inventory.push({
            key: itemKey, // Use the item key here
            amount: 1,
            damage: 0
        });
    }

    // Re-render the inventory items
    this.renderInventoryItems();
    this.displayInventoryItems();
    this.updateItemBadges();

    // Reapply the selection after re-rendering
    if (previousSelectedIndex !== null && previousSelectedIndex < this.inventory.length) {
        this.selectItem(previousSelectedIndex);
    }
},

    updateItemBadges: function() {
    document.querySelectorAll('.ui_quick_item').forEach(item => {
        const itemName = item.dataset.item;
        const badge = item.querySelector('.item-badge');

        if (badge) {
            const inventoryItem = this.inventory.find(i => i.key === itemName);
            if (inventoryItem && inventoryItem.amount > 1) {
                badge.textContent = inventoryItem.amount;
                badge.style.display = 'flex';
            } else {
                badge.style.display = 'none';
            }
        }
    });
},


    updateScale: function(element) {
    if (!element) {
        console.error('Element is null, cannot update scale.');
        return;
    }

    const icon = element.querySelector('.items_icon');
    if (icon) {
        icon.classList.remove('scale-[4]');
        icon.classList.add('scale-[2.1]');
    } else {
        console.error('Icon element not found in the inventory item.');
    }
},

    handleDragEnd: function(e) {
        const draggableItems = document.querySelectorAll('.ui_quick_item');
        draggableItems.forEach(item => {
            item.classList.remove('dragging');
            item.style.cursor = 'grab';
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
<?php 
}
?>
