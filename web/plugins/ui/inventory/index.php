<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config/db.php';
if ($auth) {
?>

<div data-close="false" class="fixed bottom-5 left-1/2 transform -translate-x-1/2 z-10 flex flex-col items-start space-y-0">


<!-- Container for L1, Tabs, and R1 -->
<div class="flex items-center mt-2">
    <!-- L1 Button -->
    <button class="text-white py-1 px-2 text-xs flex items-center">
        <div class="gamepad_button_l1"></div>
    </button>

    <!-- Tab Buttons -->
    <div id="ui_inventory_tabs" class="flex items-center">
        <button class="tab-button text-white py-1 px-2 text-xs flex items-center" data-tab="pewpew">
            Backpack
        </button>
        <button class="tab-button text-white py-1 px-2 text-xs flex items-center" data-tab="defence">
            Objectives
        </button>
        <button class="tab-button text-white py-1 px-2 text-xs flex items-center" data-tab="meals">
            Energy
        </button>
        <button class="tab-button text-white py-1 px-2 text-xs flex items-center" data-tab="random">
            Health
        </button>

        <button class="tab-button text-white py-1 px-2 text-xs flex items-center" data-tab="misc">
            Misc
        </button>
    </div>

    <!-- R1 Button -->
    <button class="text-white py-1 px-2 text-xs flex items-center">
        <div class="gamepad_button_r1"></div>
    </button>
</div>


  <!-- Inventory Slots -->
  <div id="ui_inventory_window" class="flex space-x-2 bg-[#0a0d14]/90 p-2 shadow-inner hover:shadow-lg rounded-tl-none pixel-corners">
    <div class="flex space-x-2" id="ui_quick_items_container"></div>
  </div>
  </div>

  <script>
window[id] = {
    id: id,
    inventory: [
        { name: "sword", amount: 8, category: "pewpew", damage: 10 },
        { name: "wood", amount: 12, category: "pewpew", damage: 60 },
        { name: "banana", amount: 22, category: "pewpew", damage: 60 },
        { name: "gift", amount: 0, category: "pewpew", damage: 0 },
        { name: "sweet", amount: 0, category: "pewpew", damage: 60 },
        { name: "bow_green", amount: 0, category: "pewpew", damage: 60 },
        { name: "wine", amount: 0, category: "pewpew", damage: 60 },
        { name: "potion", amount: 0, category: "pewpew", damage: 60 },
        { name: "key", amount: 0, category: "pewpew", damage: 60 },
        { name: "fish", amount: 0, category: "pewpew", damage: 60 }
    ],

    currentTab: 'pewpew',
    currentItemIndex: 0,
    lastButtonPress: 0,
    throttleDuration: 100,
    isItemSelected: false,
    targetItemIndex: null,
    dragClone: null,
    inTabSwitchingMode: false,
    currentTabButtonIndex: 0,
    isAButtonHeld: false,
    heldItem: null,
    heldCategory: null,
    heldIndex: null,
    itemsData: null,
    itemsImg: null,

    start: function() {

        assets.preload([
            { name: 'itemsImg', path: 'img/icons/items.png' },
            { name: 'itemsData', path: 'json/itemsData.json' }

        ], () => {

            this.itemsData = assets.use('itemsData');
            this.itemsImg = assets.use('itemsImg');

            this.renderInventoryItems();
            this.initializeDragAndDrop();
            this.initializeQuickItems();
            this.initializeTabDragOver();
            this.displayInventoryItems();
            this.setupGamepadEvents();

    if (this.itemsData && this.itemsImg) {
        this.displayInventoryItems();
    } else {
        console.error("itemsData or itemsImg is not loaded.");
    }

    this.checkAndUpdateUIPositions();
    this.selectItem(0);

    document.addEventListener('dragover', this.documentDragOverHandler.bind(this));
    document.addEventListener('drop', this.documentDropHandler.bind(this));

    const tabButtons = document.querySelectorAll('.tab-button');
    tabButtons.forEach(button => {
        button.addEventListener('click', (e) => {
            this.switchTab(e.target.getAttribute('data-tab'));
        });
    });

    if (tabButtons.length > 0) {
        tabButtons[0].classList.add('rounded-l-lg');
        tabButtons[tabButtons.length - 1].classList.add('rounded-r-lg');
    }

    this.clearTabHighlights();
    this.highlightSelectedTab();

});
},

    highlightSelectedTab: function() {
        const activeButton = document.querySelector(`.tab-button[data-tab="${this.currentTab}"]`);
        if (activeButton) {
            activeButton.classList.add('bg-[#3c4e6f]');
        }
    },

    switchTab: function(tabName) {
        this.currentTab = tabName;
        this.currentItemIndex = 0; 
        this.renderInventoryItems();
        this.displayInventoryItems();

        const firstItemIndex = this.getFilteredInventory().findIndex(item => item !== null);
        this.selectItem(firstItemIndex !== -1 ? firstItemIndex : 0);

        this.clearTabHighlights();
        this.highlightSelectedTab();
        this.initializeDragAndDrop();
        this.initializeQuickItems();

        this.currentTabButtonIndex = this.getTabButtons().findIndex(button => button.getAttribute('data-tab') === this.currentTab);

        // If we are in tab switching mode, override the highlight to yellow
        if (this.inTabSwitchingMode) {
            this.highlightTabButton(this.currentTabButtonIndex);
        }

        // If an item is selected, re-apply green highlight if we are in the correct category
        if (this.isItemSelected) {
            this.highlightOriginalSlotGreen();
        }
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
        gamepad.throttle((e) => this.aButton(e), this.throttleDuration);  
        gamepad.throttle((e) => this.bButton(), this.throttleDuration);
        gamepad.throttle((e) => this.upButton(e), this.throttleDuration);
        gamepad.throttle((e) => this.downButton(e), this.throttleDuration);
        gamepad.throttle((e) => this.enterTabButton(e), this.throttleDuration);
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
            if (this.inTabSwitchingMode) {
                this.currentTabButtonIndex = (this.currentTabButtonIndex - 1 + this.getTabButtons().length) % this.getTabButtons().length;
                const selectedTab = this.getTabButtons()[this.currentTabButtonIndex].getAttribute('data-tab');
                this.switchTab(selectedTab);
                audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            } else {
                this.currentItemIndex = (this.currentItemIndex - 1 + 10) % 10;  
                this.selectItem(this.currentItemIndex);
                audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            }
        });
    },

    rightButton: function(e) {
        this.throttle(() => {
            if (this.inTabSwitchingMode) {
                this.currentTabButtonIndex = (this.currentTabButtonIndex + 1) % this.getTabButtons().length;
                const selectedTab = this.getTabButtons()[this.currentTabButtonIndex].getAttribute('data-tab');
                this.switchTab(selectedTab);
                audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            } else {
                this.currentItemIndex = (this.currentItemIndex + 1) % 10; 
                this.selectItem(this.currentItemIndex);
                audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            }
        });
    },

    upButton: function(e) {
        if (!this.inTabSwitchingMode) {
            this.inTabSwitchingMode = true;
            // Highlight current tab yellow
            this.highlightTabButton(this.currentTabButtonIndex);
            audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
        }
    },

    downButton: function(e) {
        if (this.inTabSwitchingMode) {
            this.inTabSwitchingMode = false;
            this.clearTabHighlights();
            this.highlightSelectedTab();
            audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);

            if (this.isItemSelected) {
                this.highlightOriginalSlotGreen();
            }
        }
    },

    aButton: function(e) {
        this.throttle(() => {
            if (this.inTabSwitchingMode) {
                audio.playAudio("switchInventoryTab", assets.use('click'), 'sfx', false);
                return;
            }

            if (!this.isItemSelected) {
                // FIRST A PRESS: PICK UP THE ITEM
                const filteredInventory = this.getFilteredInventory();
                const selectedItem = filteredInventory[this.currentItemIndex];

                if (!selectedItem) {
                    // No item in this slot to pick up
                    audio.playAudio("error", assets.use('error'), 'sfx', false);
                    return;
                }

                // Store the held item
                this.heldItem = { ...selectedItem };
                // Remove the item from inventory
                const globalIndex = this.inventory.findIndex(i => i && i.name === selectedItem.name && i.category === selectedItem.category);
                if (globalIndex !== -1) {
                    this.inventory.splice(globalIndex, 1);
                }

                this.isItemSelected = true;
                this.heldCategory = this.currentTab; 
                this.heldIndex = this.currentItemIndex;

                // Re-render now that the item is removed
                this.renderInventoryItems();
                this.displayInventoryItems();
                this.updateItemBadges();

                // Highlight the original slot green
                this.highlightOriginalSlotGreen();

                audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            } else {
                // SECOND A PRESS: DROP/SWAP THE ITEM
                const filteredInventory = this.getFilteredInventory();
                const hoveredItem = filteredInventory[this.currentItemIndex];

                if (hoveredItem) {
                    // The hovered slot is occupied, swap items
                    const hoveredItemCopy = { ...hoveredItem };
                    const hoveredGlobalIndex = this.getGlobalInventoryIndexForSlot(this.currentTab, this.currentItemIndex);
                    if (hoveredGlobalIndex !== -1) this.inventory.splice(hoveredGlobalIndex, 1);

                    // Place heldItem in hovered slot
                    this.inventory.push({
                        ...this.heldItem,
                        category: this.currentTab
                    });

                    // Place hovered item back to the original slot
                    this.inventory.push({
                        ...hoveredItemCopy,
                        category: this.heldCategory
                    });
                } else {
                    // The hovered slot is empty, just place heldItem here
                    this.inventory.push({
                        ...this.heldItem,
                        category: this.currentTab
                    });
                }

                // Clear selection
                this.isItemSelected = false;
                this.heldItem = null;
                this.heldCategory = null;
                this.heldIndex = null;

                this.renderInventoryItems();
                this.displayInventoryItems();
                this.updateItemBadges();
                this.selectItem(this.currentItemIndex);

                audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            }
        });
    },

    bButton: function(e) {
        this.throttle(() => {
            if (this.isItemSelected) {
                // Cancel the selection, return heldItem back to original slot
                this.inventory.push({
                    ...this.heldItem,
                    category: this.heldCategory
                });

                this.isItemSelected = false;
                this.heldItem = null;
                this.heldCategory = null;
                this.heldIndex = null;

                this.renderInventoryItems();
                this.displayInventoryItems();
                this.updateItemBadges();
                this.selectItem(this.currentItemIndex);

                audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            } else {
                console.log('B button pressed, no swap mode active.');
                audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            }
        });
    },

    enterTabButton: function(e) {
        // Not strictly needed now.
    },

    highlightTabButton: function(index) {
        const tabButtons = this.getTabButtons();
        tabButtons.forEach((button) => {
            if (!button.classList.contains('bg-[#3c4e6f]')) {
                button.classList.remove('bg-yellow-500');
                button.classList.add('bg-[#0a0d14]');
            }
        });
        tabButtons[index].classList.add('bg-yellow-500');
    },

    getTabButtons: function() {
        return Array.from(document.querySelectorAll('.tab-button'));
    },

    highlightOriginalSlotGreen: function() {
    if (this.isItemSelected && this.currentTab === this.heldCategory) {
        const itemElements = document.querySelectorAll('.ui_quick_item');
        const originalSlotElement = itemElements[this.heldIndex];
        if (originalSlotElement) {
            // Directly highlight green, do not call clearHighlights here.
            originalSlotElement.classList.add('border-2', 'border-green-500');
        }
    }
},


clearTabHighlights: function() {
    document.querySelectorAll('.tab-button').forEach(button => {
        button.classList.remove('bg-[#3c4e6f]', 'bg-yellow-500', 'animate-pulse');
        button.classList.add('bg-[#0a0d14]');
    });
},

clearHighlights: function(clearBlueBackground = true) {
    const draggableItems = document.querySelectorAll('.ui_quick_item');
    draggableItems.forEach((item) => {
        item.classList.remove(
            'border-2',
            'border-dashed',
            'border-yellow-500',
            'border-green-500',
            'animate-pulse' // Remove pulse from slots
        );
        if (clearBlueBackground) {
            item.style.backgroundColor = '';
        }
    });

    // After clearing, if we have a selected item, highlight its original slot green
    if (this.isItemSelected) {
        this.highlightOriginalSlotGreen();
    }
},

    selectItem: function(index) {
        this.clearHighlights();
        const itemElements = document.querySelectorAll('.ui_quick_item');
        const itemElement = itemElements[index];
        if (itemElement) {
            itemElement.classList.add('border-2', 'border-dashed', 'border-yellow-500');
        }

        if (this.isItemSelected) {
            this.highlightOriginalSlotGreen();
        }

        const filteredInventory = this.getFilteredInventory();
        const selectedItem = filteredInventory[index];
        const sprite = game.sprites[game.playerid];
        if (selectedItem && sprite) {
            sprite.currentItem = selectedItem.name;
        } else if (sprite) {
            sprite.currentItem = null;
        }
    },

    swapItems: function() {
        // Not used in this updated logic since we handle swap in A press directly.
    },

    renderInventoryItems: function() {
        const quickItemsContainer = document.getElementById('ui_quick_items_container');
        quickItemsContainer.innerHTML = '';

        const filteredItems = this.getFilteredInventory();

        for (let i = 0; i < 10; i++) {
            const item = filteredItems[i];
            const itemElement = document.createElement('div');
            itemElement.className = 'ui_quick_item relative cursor-move w-14 h-14 bg-[#18202f]/20 rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center';
            itemElement.dataset.item = item ? item.name : '';

            if (item) {
                const condition = 100 - (item.damage || 0); 
                let barColor = 'bg-green-500';
                if (condition <= 10) {
                    barColor = 'bg-red-500';
                } else if (condition <= 50) {
                    barColor = 'bg-orange-500';
                }

                itemElement.innerHTML = `
                    <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
                    <div class="items_icon items_${item.name} scale-[2.1] z-10"></div>
                    ${item.amount > 1 ? `
                    <div class="item-badge absolute top-0 left-0 z-20 bg-[#18202f] text-white rounded-full text-xs w-5 h-5 flex items-center justify-center">
                        ${item.amount}
                    </div>
                    ` : ''}
                    <div class="damage-bar absolute bottom-0 left-0 right-0 h-1 ${barColor} rounded-full" style="width: ${condition}%;"></div>
                `;
            } else {
                itemElement.innerHTML = `
                    <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
                `;
            }

            quickItemsContainer.appendChild(itemElement);
        }
    },

    getFilteredInventory: function() {
        const filtered = this.inventory.filter(item => item.category === this.currentTab);
        const emptySlots = Array(10 - filtered.length).fill(null); 
        return [...filtered, ...emptySlots];
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
        if (!this.itemsData || !this.itemsData.items) {
            console.error("itemsData or items array is not defined.");
            return;
        }

        this.getFilteredInventory().forEach((item) => {
            if (!item) return;
            const itemData = this.itemsData.items.find(data => data.name === item.name);
            if (itemData) {
                let itemElement = document.querySelector(`.ui_quick_item[data-item="${item.name}"]`);
                if (itemElement) {
                    this.setItemIcon(itemElement, itemData);
                    itemElement.dataset.cd = itemData.cd;
                    itemElement.querySelector('.items_icon').classList.add(`items_${item.name}`);
                }
            } else {
                // This might be fine if not all items have itemData
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

            if (this.itemsImg && this.itemsImg instanceof HTMLImageElement) {
                ctx.drawImage(
                    this.itemsImg, 
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
            } else {
                console.error("Invalid or unloaded image source.");
            }
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
    const draggableItems = document.querySelectorAll('.ui_quick_item');

    draggableItems.forEach(item => {
        // Desktop drag events
        item.setAttribute('draggable', true);
        item.style.cursor = 'grab';
        item.addEventListener('mouseover', this.handleMouseOver.bind(this));
        item.addEventListener('mouseout', this.handleMouseOut.bind(this));
        item.addEventListener('dragstart', this.handleDragStart.bind(this));
        item.addEventListener('dragover', this.handleDragOver.bind(this));
        item.addEventListener('drop', this.handleDrop.bind(this));
        item.addEventListener('dragend', this.handleDragEnd.bind(this));

        // Mobile touch events
        item.addEventListener('touchstart', this.handleTouchStart.bind(this), { passive: false });
        // We attach touchmove and touchend to the document to track the movement and final drop globally
    });

    // Add global listeners for touchmove and touchend to handle drag logic outside initial element
    document.addEventListener('touchmove', this.handleTouchMove.bind(this), { passive: false });
    document.addEventListener('touchend', this.handleTouchEnd.bind(this), { passive: false });
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

    initializeTabDragOver: function() {
    const tabButtons = document.querySelectorAll('.tab-button');
    const tabContainer = document.getElementById('ui_inventory_tabs');
    
    // Make tab buttons droppable (already was, but we'll ensure it's clear)
    tabButtons.forEach(button => {
        button.addEventListener('dragover', (e) => {
            e.preventDefault();
            const hoveredTab = button.getAttribute('data-tab');
            if (hoveredTab && hoveredTab !== this.currentTab) {
                this.switchTab(hoveredTab);
                this.initializeDragAndDrop();
                this.initializeQuickItems();
                this.initializeTabDragOver();
                this.selectItem(this.currentItemIndex);
                audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            }
        });
    });
    
    // **NEW**: Make the entire tab container a droppable area
    // If you drop here, we will place the item in the currently active tab category.
    tabContainer.addEventListener('dragover', (e) => {
        e.preventDefault();
        // We don't automatically switch tabs when hovering over the container
        // Just allow dropping here.
    });

    tabContainer.addEventListener('drop', (e) => {
        e.preventDefault();
        e.stopPropagation();

        if (!this.draggedItemData) return;

        // Remove the source item from the inventory
        const sourceItemInInventory = this.inventory.find(i => 
            i && i.name === this.draggedItemData.name && i.category === this.draggedItemData.category
        );
        if (sourceItemInInventory) {
            const sourceIndex = this.inventory.indexOf(sourceItemInInventory);
            if (sourceIndex !== -1) {
                this.inventory.splice(sourceIndex, 1);
            }
        }

        // If dropping on the tab container itself (not on a specific slot),
        // just put the item into the currently active category in the next available slot.
        this.draggedItemData.category = this.currentTab;
        this.inventory.push(this.draggedItemData);

        // Clear the dragged item data
        this.draggedItemData = null;
        this.dragSrcEl = null;

        // Re-render and re-initialize
        this.renderInventoryItems();
        this.displayInventoryItems();
        this.updateItemBadges();
        this.initializeDragAndDrop();
        this.initializeQuickItems();
        this.initializeTabDragOver();

        // Re-select the current item to restore highlight
        this.selectItem(this.currentItemIndex);

        audio.playAudio("slotDrop", assets.use('slotDrop'), 'sfx', false);
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

    handleTouchStart: function(e) {
    e.preventDefault();
    e.stopPropagation();

    const touch = e.touches[0];
    const target = e.target.closest('.ui_quick_item');
    if (!target || !target.dataset.item) return;

    // Imitate drag start logic
    this.dragSrcEl = target;

    const itemName = target.dataset.item;
    const sourceItem = this.inventory.find(i => i && i.name === itemName && i.category === this.currentTab);

    if (sourceItem) {
        this.draggedItemData = { ...sourceItem };

        // Get the icon before clearing the slot
        const iconDiv = target.querySelector('.items_icon');

        // Create the drag clone
        if (iconDiv) {
            const clonedIcon = iconDiv.cloneNode(true);
            const dragWrapper = document.createElement('div');
            dragWrapper.style.position = 'absolute';
            dragWrapper.style.top = `${touch.clientY}px`;
            dragWrapper.style.left = `${touch.clientX}px`;
            dragWrapper.style.pointerEvents = 'none';
            dragWrapper.style.zIndex = '1000';
            clonedIcon.style.transform = 'scale(4)';
            dragWrapper.appendChild(clonedIcon);
            this.dragClone = dragWrapper;
            document.body.appendChild(dragWrapper);
        }

        // Clear the slot and highlight
        target.innerHTML = '';
        target.classList.add(
            'bg-[#2c3e50]',
            'border-2',
            'border-dashed',
            'border-yellow-500',
            'opacity-75'
        );
    }
},

handleTouchMove: function(e) {
    if (!this.draggedItemData || !this.dragClone) return;
    e.preventDefault();
    e.stopPropagation();

    const touch = e.touches[0];
    // Move the clone
    this.dragClone.style.top = `${touch.clientY}px`;
    this.dragClone.style.left = `${touch.clientX}px`;

    // Determine what element is currently under the finger
    const target = document.elementFromPoint(touch.clientX, touch.clientY);
    if (!target) return;

    // Clear previous highlights
    this.clearHighlights();

    const tabButton = target.closest('.tab-button');
    const slot = target.closest('.ui_quick_item');

    if (tabButton) {
        const hoveredTab = tabButton.getAttribute('data-tab');
        if (hoveredTab && hoveredTab !== this.currentTab) {
            this.switchTab(hoveredTab);
            this.initializeDragAndDrop();
            this.initializeQuickItems();
            this.selectItem(this.currentItemIndex);
            audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
        }

        if (!tabButton.classList.contains('bg-yellow-500') && !tabButton.classList.contains('bg-[#3c4e6f]')) {
            tabButton.classList.add('bg-yellow-500');
        }
        return;
    }

    if (slot) {
        slot.classList.add(
            'border-2',
            'border-dashed',
            'border-yellow-500',
            'animate-pulse'
        );

        if (!this.hasPlayedDragOverSound || this.lastHoveredSlot !== slot) {
            audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            this.hasPlayedDragOverSound = true;
            this.lastHoveredSlot = slot;
        }
    } else {
        this.hasPlayedDragOverSound = false;
        this.lastHoveredSlot = null;
    }
},

handleTouchEnd: function(e) {
    if (!this.draggedItemData) return;
    e.preventDefault();
    e.stopPropagation();

    const touch = e.changedTouches[0];
    const target = document.elementFromPoint(touch.clientX, touch.clientY);
    let targetSlot = null;
    let targetTab = null;

    if (target) {
        targetSlot = target.closest('.ui_quick_item');
        targetTab = target.closest('.tab-button');
    }

    // Remove the source item from the inventory if it exists
    const sourceItemInInventory = this.inventory.find(i => i && i.name === this.draggedItemData.name && i.category === this.draggedItemData.category);
    if (sourceItemInInventory) {
        const sourceIndex = this.inventory.indexOf(sourceItemInInventory);
        if (sourceIndex !== -1) {
            this.inventory.splice(sourceIndex, 1);
        }
    }

    if (targetTab) {
        const hoveredTab = targetTab.getAttribute('data-tab');
        if (hoveredTab && hoveredTab !== this.currentTab) {
            this.switchTab(hoveredTab);
        }
        this.draggedItemData.category = this.currentTab;
        this.inventory.push(this.draggedItemData);
    } else if (targetSlot) {
        const targetIndex = Array.from(document.querySelectorAll('.ui_quick_item')).indexOf(targetSlot);
        const filteredInventory = this.getFilteredInventory();
        const targetInventoryIndex = (targetIndex !== -1 && filteredInventory[targetIndex])
            ? this.inventory.indexOf(filteredInventory[targetIndex])
            : -1;

        this.draggedItemData.category = this.currentTab;

        if (targetInventoryIndex !== -1) {
            // Target slot occupied: swap items
            const targetItem = this.inventory[targetInventoryIndex];
            this.inventory[targetInventoryIndex] = this.draggedItemData;
            this.inventory.push(targetItem);
        } else {
            // Target slot empty: just place the dragged item
            this.inventory.push(this.draggedItemData);
        }
    } else {
        // Dropped outside valid areas, revert to the current category
        this.draggedItemData.category = this.currentTab;
        this.inventory.push(this.draggedItemData);
    }

    // Clear the dragged item data
    this.draggedItemData = null;
    this.dragSrcEl = null;

    if (this.dragClone) {
        document.body.removeChild(this.dragClone);
        this.dragClone = null;
    }

    // Re-render the inventory
    this.renderInventoryItems();
    this.displayInventoryItems();
    this.updateItemBadges();
    this.initializeDragAndDrop();
    this.initializeQuickItems();
    this.initializeTabDragOver();
    this.selectItem(this.currentItemIndex);

    audio.playAudio("slotDrop", assets.use('slotDrop'), 'sfx', false);
},

    handleDragStart: function(e) {
    this.dragSrcEl = e.target.closest('.ui_quick_item');

    if (!this.dragSrcEl || !this.dragSrcEl.dataset.item) {
        e.preventDefault();
        return;
    }

    const itemName = this.dragSrcEl.dataset.item;
    const sourceItem = this.inventory.find(i => i && i.name === itemName && i.category === this.currentTab);

    if (sourceItem) {
        this.draggedItemData = { ...sourceItem };
        e.dataTransfer.effectAllowed = 'move';

        // Get the icon before clearing the slot
        const iconDiv = this.dragSrcEl.querySelector('.items_icon');

        // Create the drag clone first
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

        // Now clear the slot and apply Tailwind classes for highlight
        this.dragSrcEl.innerHTML = '';
        this.dragSrcEl.classList.add(
            'bg-[#2c3e50]',
            'border-2',
            'border-dashed',
            'border-yellow-500',
            'opacity-75'
        );

        e.target.style.cursor = 'grabbing';

        // Use an empty image to avoid the default browser drag image
        var img = new Image();
        img.src = '';
        e.dataTransfer.setDragImage(img, 0, 0);
    } else {
        // If the item isn't found, prevent dragging
        e.preventDefault();
    }
},


handleDragOver: function(e) {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';

    const target = e.target.closest('.ui_quick_item, .tab-button');
    if (this.dragClone) {
        this.dragClone.style.top = `${e.clientY}px`;
        this.dragClone.style.left = `${e.clientX}px`;
    }

    // Clear previous highlights
    this.clearHighlights();

    if (target && target.classList.contains('tab-button')) {
        // If hovering over a tab button, switch to that tab if needed
        const hoveredTab = target.getAttribute('data-tab');
        if (hoveredTab && hoveredTab !== this.currentTab) {
            this.switchTab(hoveredTab);

            // Re-initialize drag and drop after switching tabs
            this.initializeDragAndDrop();
            this.initializeQuickItems();
            this.selectItem(this.currentItemIndex);

            audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
        }

        // Highlight tab button if not already active or in tab switching mode
        if (!target.classList.contains('bg-yellow-500') && !target.classList.contains('bg-[#3c4e6f]')) {
            target.classList.add('bg-yellow-500');
        }
        return false;
    }

    // If hovering over an inventory slot
    if (target && target.classList.contains('ui_quick_item')) {
        // Add border and pulsation
        target.classList.add(
            'border-2',
            'border-dashed',
            'border-yellow-500',
            'animate-pulse'
        );

        if (!this.hasPlayedDragOverSound || this.lastHoveredSlot !== target) {
            audio.playAudio("menuDrop", assets.use('menuDrop'), 'sfx', false);
            this.hasPlayedDragOverSound = true;
            this.lastHoveredSlot = target;
        }
    } else {
        // If not hovering over a valid slot or tab, reset.
        this.hasPlayedDragOverSound = false;
        this.lastHoveredSlot = null;
    }

    return false;
},


handleDrop: function(e) {
    e.stopPropagation();
    e.preventDefault();

    const targetSlot = e.target.closest('.ui_quick_item');
    const targetTab = e.target.closest('.tab-button');

    if (!this.draggedItemData) {
        return false;
    }

    // Remove the source item from the inventory if it exists
    const sourceItemInInventory = this.inventory.find(i => i && i.name === this.draggedItemData.name && i.category === this.draggedItemData.category);
    if (sourceItemInInventory) {
        const sourceIndex = this.inventory.indexOf(sourceItemInInventory);
        if (sourceIndex !== -1) {
            this.inventory.splice(sourceIndex, 1);
        }
    }

    if (targetTab) {
        const hoveredTab = targetTab.getAttribute('data-tab');
        if (hoveredTab && hoveredTab !== this.currentTab) {
            this.switchTab(hoveredTab);
        }
        this.draggedItemData.category = this.currentTab;
        this.inventory.push(this.draggedItemData);
    } else if (targetSlot) {
        const targetIndex = Array.from(document.querySelectorAll('.ui_quick_item')).indexOf(targetSlot);
        const filteredInventory = this.getFilteredInventory();
        const targetInventoryIndex = (targetIndex !== -1 && filteredInventory[targetIndex])
            ? this.inventory.indexOf(filteredInventory[targetIndex])
            : -1;

        this.draggedItemData.category = this.currentTab;

        if (targetInventoryIndex !== -1) {
            // Target slot occupied: swap items
            const targetItem = this.inventory[targetInventoryIndex];
            this.inventory[targetInventoryIndex] = this.draggedItemData;
            this.inventory.push(targetItem);
        } else {
            // Target slot empty: just place the dragged item
            this.inventory.push(this.draggedItemData);
        }
    } else {
        // Dropped outside valid areas, revert to the current category
        this.draggedItemData.category = this.currentTab;
        this.inventory.push(this.draggedItemData);
    }

    // Clear the dragged item data
    this.draggedItemData = null;
    this.dragSrcEl = null;

    // Re-render the inventory
    this.renderInventoryItems();
    this.displayInventoryItems();
    this.updateItemBadges();
    this.initializeDragAndDrop();
    this.initializeQuickItems();
    this.initializeTabDragOver();
    this.selectItem(this.currentItemIndex);

    audio.playAudio("slotDrop", assets.use('slotDrop'), 'sfx', false);
    return false;
},


handleDragEnd: function(e) {
    const draggableItems = document.querySelectorAll('.ui_quick_item');
    draggableItems.forEach(item => {
        item.style.cursor = 'grab';
    });

    if (this.dragClone) {
        document.body.removeChild(this.dragClone);
        this.dragClone = null;
    }

    // On drag end, re-render the inventory to restore slot states
    this.renderInventoryItems();
    this.displayInventoryItems();
    this.updateItemBadges();
    this.initializeDragAndDrop();
    this.initializeQuickItems();
    this.initializeTabDragOver();
    this.selectItem(this.currentItemIndex);
},

    updateItemBadges: function() {
        document.querySelectorAll('.ui_quick_item').forEach(item => {
            const itemName = item.dataset.item;
            const badge = item.querySelector('.item-badge');
            if (badge) {
                const inventoryItem = this.inventory.find(i => i.name === itemName && i.category === this.currentTab);
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

    getGlobalInventoryIndexForSlot: function(category, slotIndex) {
        const filtered = this.inventory.filter(item => item.category === category);
        if (slotIndex < filtered.length && filtered[slotIndex]) {
            const item = filtered[slotIndex];
            return this.inventory.indexOf(item);
        }
        return -1;
    },
};
  </script>
<?php 
}
?>
