<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>

<div data-window='ui_inventory_window' data-close="false" class="fixed bottom-5 left-1/2 transform -translate-x-1/2 z-10 flex flex-col items-start space-y-0">
  
  <!-- Tab Buttons -->
  <div id="ui_inventory_tabs" class="flex bg-[#0a0d14] border border-black border-b-0 rounded-tl-lg rounded-tr-lg">
    <button class="tab-button text-white py-1 px-2 text-xs flex items-center border-r border-black rounded-tl-lg" data-tab="pewpew">
        Pew Pew
    </button>
    <button class="tab-button text-white py-1 px-2 text-xs flex items-center border-r border-black" data-tab="armour">
        Armour
    </button>
    <button class="tab-button text-white py-1 px-2 text-xs flex items-center border-r border-black" data-tab="defence">
        Defence
    </button>
    <button class="tab-button text-white py-1 px-2 text-xs flex items-center border-r border-black" data-tab="meals">
        Nutritious Meals
    </button>
    <button class="tab-button text-white py-1 px-2 text-xs flex items-center rounded-tr-lg" data-tab="random">
        Random shit lol
    </button>
</div>

  <!-- Inventory Slots -->
  <div id="ui_inventory_window" class="flex space-x-2 bg-[#0a0d14] p-2 shadow-inner hover:shadow-lg border border-black rounded-lg rounded-tl-none">
    <div class="flex space-x-2" id="ui_quick_items_container"></div>
  </div>

  <script>
var ui_inventory_window = {
    inventories: {
        pewpew: [
            { name: "sword", amount: 3 },
            { name: "fireball", amount: 7 },
            { name: "shield", amount: 2 },
            { name: "arrow_green", amount: 5 },
            { name: "bow_green", amount: 1 },
            { name: "psychic", amount: 4 },
            { name: "key", amount: 1 },
            { name: "skull", amount: 2 },
            { name: "sweet", amount: 6 },
            { name: "banana", amount: 8 },
            { name: "apple", amount: 9 },
            { name: "energy", amount: 10 },
            { name: "health", amount: 7 },
            { name: "fire", amount: 4 },
            { name: "wood", amount: 12 }
        ],
        armour: [
            { name: "shield", amount: 5 },
            { name: "helmet", amount: 1 },
            { name: "chestplate", amount: 1 },
            { name: "leggings", amount: 1 },
            { name: "boots", amount: 1 },
            { name: "green_emerald", amount: 2 },
            { name: "yellow_emerald", amount: 2 },
            { name: "blue_emerald", amount: 2 },
            { name: "red_emerald", amount: 2 },
            { name: "silver_emerald", amount: 2 },
            { name: "pink_emerald", amount: 1 },
            { name: "orange_emerald", amount: 3 },
            { name: "brown_emerald", amount: 1 },
            { name: "purple_emerald", amount: 2 },
            { name: "black_emerald", amount: 1 }
        ],
        defence: [
            { name: "shield", amount: 2 },
            { name: "trap", amount: 10 },
            { name: "wall", amount: 5 },
            { name: "skull", amount: 3 },
            { name: "wood", amount: 12 },
            { name: "potion", amount: 1 },
            { name: "fire", amount: 3 },
            { name: "wine", amount: 2 },
            { name: "banana", amount: 10 },
            { name: "fish", amount: 8 },
            { name: "gift", amount: 1 },
            { name: "health", amount: 5 },
            { name: "energy", amount: 6 },
            { name: "sweet", amount: 7 },
            { name: "key", amount: 2 }
        ],
        meals: [
            { name: "banana", amount: 99 },
            { name: "apple", amount: 99 },
            { name: "fish", amount: 13 },
            { name: "sweet", amount: 4 },
            { name: "potion", amount: 1 },
            { name: "wine", amount: 5 },
            { name: "gift", amount: 2 },
            { name: "wood", amount: 7 },
            { name: "skull", amount: 3 },
            { name: "energy", amount: 6 },
            { name: "health", amount: 8 },
            { name: "fire", amount: 2 },
            { name: "fireball", amount: 4 },
            { name: "sword", amount: 2 },
            { name: "shield", amount: 3 }
        ],
        random: [
            { name: "skull", amount: 3 },
            { name: "wood", amount: 99 },
            { name: "gift", amount: 7 },
            { name: "gold", amount: 28 },
            { name: "banana", amount: 11 },
            { name: "apple", amount: 13 },
            { name: "fish", amount: 9 },
            { name: "sweet", amount: 5 },
            { name: "potion", amount: 3 },
            { name: "wine", amount: 2 },
            { name: "green_emerald", amount: 6 },
            { name: "red_emerald", amount: 7 },
            { name: "yellow_emerald", amount: 8 },
            { name: "pink_emerald", amount: 1 },
            { name: "brown_emerald", amount: 1 }
        ]
    },

    currentTab: 'pewpew',
    currentItemIndex: 0,
    lastButtonPress: 0,
    throttleDuration: 150,
    isItemSelected: false,
    targetItemIndex: null,
    dragClone: null,
    inTabSwitchingMode: false,
    currentTabButtonIndex: 0,

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
        // Apply rounded corners to the first and last tab buttons
        tabButtons[0].classList.add('rounded-tl-lg'); // First tab button
        tabButtons[tabButtons.length - 1].classList.add('rounded-tr-lg'); // Last tab button
    }

    this.clearTabHighlights();
    this.highlightSelectedTab();  // Ensure the active tab is highlighted on load
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
    this.currentItemIndex = 0;
    this.renderInventoryItems();
    this.displayInventoryItems();
    this.selectItem(0);

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
    gamepad.throttle((e) => this.enterTabButton(e), this.throttleDuration);
},

    switchToGamepadMode: function() {
        console.log("Switched to gamepad mode.");
        this.selectItem(0);
    },

    leftButton: function(e) {
    this.throttle(() => {
        if (this.inTabSwitchingMode) {
            // Navigate left through tabs
            this.currentTabButtonIndex = (this.currentTabButtonIndex - 1 + this.getTabButtons().length) % this.getTabButtons().length;
            this.clearTabHighlights();  // Clear all highlights
            this.highlightSelectedTab();  // Keep the active tab grey
            this.highlightTabButton(this.currentTabButtonIndex);  // Highlight the new tab with yellow
        } else {
            // Navigate left through items
            this.currentItemIndex = (this.currentItemIndex - 1 + this.inventories[this.currentTab].length) % this.inventories[this.currentTab].length;
            console.log(`Current item index: ${this.currentItemIndex}`);
            audio.playAudio("menuSelect", assets.load('menuSelect'), 'sfx', false);
            this.selectItem(this.currentItemIndex);
        }
    });
},

rightButton: function(e) {
    this.throttle(() => {
        if (this.inTabSwitchingMode) {
            // Navigate right through tabs
            this.currentTabButtonIndex = (this.currentTabButtonIndex + 1) % this.getTabButtons().length;
            this.clearTabHighlights();  // Clear all highlights
            this.highlightSelectedTab();  // Keep the active tab grey
            this.highlightTabButton(this.currentTabButtonIndex);  // Highlight the new tab with yellow
        } else {
            // Navigate right through items
            this.currentItemIndex = (this.currentItemIndex + 1) % this.inventories[this.currentTab].length;
            console.log(`Current item index: ${this.currentItemIndex}`);
            audio.playAudio("menuSelect", assets.load('menuSelect'), 'sfx', false);
            this.selectItem(this.currentItemIndex);
        }
    });
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

    upButton: function(e) {
    if (!this.inTabSwitchingMode) {
        this.inTabSwitchingMode = true;
        this.highlightTabButton(this.currentTabButtonIndex);
    }
},

downButton: function(e) {
    if (this.inTabSwitchingMode) {
        this.inTabSwitchingMode = false;
        this.clearTabHighlights();
        this.highlightSelectedTab();  // Ensure the active tab remains grey
    }
},

    aButton: function(e) {
    if (this.inTabSwitchingMode) {
        this.enterTabButton(e);  // Simulates the "Enter" button behavior
    }
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

        const temp = this.inventories[this.currentTab][selectedItemIndex];
        this.inventories[this.currentTab][selectedItemIndex] = this.inventories[this.currentTab][targetIndex];
        this.inventories[this.currentTab][targetIndex] = temp;

        const selectedElement = document.querySelector(`.ui_quick_item[data-item="${this.inventories[this.currentTab][targetIndex].name}"]`);
        const targetElement = document.querySelector(`.ui_quick_item[data-item="${temp.name}"]`);

        if (selectedElement && targetElement) {
            const tempInnerHTML = selectedElement.innerHTML;
            selectedElement.innerHTML = targetElement.innerHTML;
            targetElement.innerHTML = tempInnerHTML;

            selectedElement.dataset.item = this.inventories[this.currentTab][targetIndex].name;
            targetElement.dataset.item = temp.name;

            this.currentItemIndex = targetIndex;
        }

        this.clearHighlights();
        this.isItemSelected = false;
        this.targetItemIndex = null;
    },

    highlightTargetItem: function(direction) {
        this.clearHighlights(false);

        let newIndex;

        if (direction === 'left') {
            newIndex = (this.currentItemIndex - 1 + this.inventories[this.currentTab].length) % this.inventories[this.currentTab].length;
        } else if (direction === 'right') {
            newIndex = (this.currentItemIndex + 1) % this.inventories[this.currentTab].length;
        }

        this.targetItemIndex = newIndex;

        let targetItem = document.querySelector(`.ui_quick_item[data-item="${this.inventories[this.currentTab][this.targetItemIndex].name}"]`);
        if (targetItem) {
            targetItem.classList.add('border-2', 'border-green-500');
        }
    },

    highlightSelectedItem: function() {
        this.clearHighlights();
        let selectedItem = document.querySelector(`.ui_quick_item[data-item="${this.inventories[this.currentTab][this.currentItemIndex].name}"]`);

        if (selectedItem) {
            selectedItem.style.backgroundColor = 'blue';
        }
    },

    selectItem: function(index) {
        this.clearHighlights();

        const itemElement = document.querySelector(`.ui_quick_item[data-item="${this.inventories[this.currentTab][index].name}"]`);
        if (itemElement) {
            itemElement.classList.add('border-2', 'border-dashed', 'border-yellow-500');
        }

        document.querySelectorAll('.ui_quick_item').forEach(item => {
            this.updateScale(item);
        });
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

        this.inventories[this.currentTab].forEach(item => {
            const itemElement = document.createElement('div');
            itemElement.className = 'ui_quick_item relative cursor-move w-14 h-14 bg-[#18202f] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center';
            itemElement.dataset.item = item.name;
            itemElement.innerHTML = `
                <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
                <div class="items_icon items_${item.name} scale-[2.1] z-10"></div>
                ${item.amount > 1 ? `
                <div class="item-badge absolute top-0 left-0 z-20 bg-[#18202f] text-white rounded-full text-xs w-5 h-5 flex items-center justify-center">
                    ${item.amount}
                </div>
                ` : ''}
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
                const targetObject = game.findObjectAt(mouseX, mouseY);

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
        if (!game.itemsData || !game.itemsData.items) {
            console.error("itemsData or items array is not defined.");
            return;
        }

        this.inventories[this.currentTab].forEach((item) => {
            const itemData = game.itemsData.items.find(data => data.name === item.name);
            if (itemData) {
                let itemElement = document.querySelector(`.ui_quick_item[data-item="${item.name}"]`);

                if (itemElement) {
                    this.setItemIcon(itemElement, itemData);
                    itemElement.dataset.cd = itemData.cd;
                    itemElement.querySelector('.items_icon').classList.add(`items_${item.name}`);
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

            if (game.itemsImg && game.itemsImg instanceof HTMLImageElement) {
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
        const target = e.target.closest('.ui_quick_item');
        if (this.dragSrcEl !== target && target) {
            const tempInnerHTML = this.dragSrcEl.innerHTML;
            this.dragSrcEl.innerHTML = target.innerHTML;
            target.innerHTML = tempInnerHTML;

            const tempDataItem = this.dragSrcEl.dataset.item;
            this.dragSrcEl.dataset.item = target.dataset.item;
            target.dataset.item = tempDataItem;

            const srcIndex = this.inventories[this.currentTab].findIndex(item => item.name === tempDataItem);
            const targetIndex = this.inventories[this.currentTab].findIndex(item => item.name === this.dragSrcEl.dataset.item);
            [this.inventories[this.currentTab][srcIndex], this.inventories[this.currentTab][targetIndex]] = [this.inventories[this.currentTab][targetIndex], this.inventories[this.currentTab][srcIndex]];

            if (this.currentItemIndex === srcIndex) {
                this.currentItemIndex = targetIndex;
            }

            this.updateScale(this.dragSrcEl);
            this.updateScale(target);

            this.updateItemBadges();

            this.clearHighlights();
            this.selectItem(this.currentItemIndex);

            audio.playAudio("sceneDrop", assets.load('sceneDrop'), 'sfx', false);
        } else {
            audio.playAudio("slotDrop", assets.load('slotDrop'), 'sfx', false);
        }
        return false;
    },

    updateItemBadges: function() {
        document.querySelectorAll('.ui_quick_item').forEach(item => {
            const itemName = item.dataset.item;
            const badge = item.querySelector('.item-badge');
            if (badge) {
                if (this.inventories[this.currentTab].find(i => i.name === itemName).amount > 1) {
                    badge.textContent = this.inventories[this.currentTab].find(i => i.name === itemName).amount;
                    badge.style.display = 'flex';
                } else {
                    badge.style.display = 'none';
                }
            }
        });
    },

    updateScale: function(element) {
        const icon = element.querySelector('.items_icon');
        icon.classList.remove('scale-[4]');
        icon.classList.add('scale-[2.1]');
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
