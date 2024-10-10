<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>

<div data-window='ui_inventory_window' data-close="false" class="fixed bottom-5 left-1/2 transform -translate-x-1/2 z-10 flex flex-col items-start space-y-0">
  
  <!-- Inventory Slots -->
  <div id="ui_inventory_window" class="flex space-x-2 bg-[#0a0d14] p-2 shadow-inner hover:shadow-lg border border-black rounded-lg">
    <div class="flex space-x-2" id="ui_quick_items_container"></div>
  </div>

  <script>
var ui_inventory_window = {
    inventory: [
        { name: "sword", amount: 0, damage: 60 }
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

        this.checkAndUpdateUIPositions();
        this.selectItem(0);

        document.addEventListener('dragover', this.documentDragOverHandler.bind(this));
        document.addEventListener('drop', this.documentDropHandler.bind(this));
        modal.front('ui_inventory_window');
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
        gamepad.throttle(() => this.bButton(), this.throttleDuration);

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
            this.currentItemIndex = (this.currentItemIndex - 1 + 15) % 15;
            this.selectItem(this.currentItemIndex);
            audio.playAudio("menuDrop", assets.load('menuDrop'), 'sfx', false);
        });
    },

    rightButton: function(e) {
        this.throttle(() => {
            this.currentItemIndex = (this.currentItemIndex + 1) % 15;
            this.selectItem(this.currentItemIndex);
            audio.playAudio("menuDrop", assets.load('menuDrop'), 'sfx', false);
        });
    },

    upButton: function(e) {
        if (!this.inTabSwitchingMode) {
            this.inTabSwitchingMode = true;
            audio.playAudio("menuDrop", assets.load('menuDrop'), 'sfx', false);
        }
    },

    downButton: function(e) {
        if (this.inTabSwitchingMode) {
            this.inTabSwitchingMode = false;
            audio.playAudio("menuDrop", assets.load('menuDrop'), 'sfx', false);
        }
    },

    aButton: function(e) {
        this.throttle(() => {
            if (!this.isItemSelected) {
                this.isItemSelected = true;
                this.highlightSelectedItem();
                this.targetItemIndex = this.currentItemIndex;
                audio.playAudio("menuDrop", assets.load('menuDrop'), 'sfx', false);
            } else {
                this.swapItems();
                this.isItemSelected = false;
                this.clearHighlights();
                this.selectItem(this.currentItemIndex);
                audio.playAudio("menuDrop", assets.load('menuDrop'), 'sfx', false);
            }
        });
    },

    bButton: function(e) {
        this.throttle(() => {
            if (this.isItemSelected) {
                this.isItemSelected = false;
                this.clearHighlights();
            } else {
                console.log('B button pressed, no swap mode active.');
                audio.playAudio("menuDrop", assets.load('menuDrop'), 'sfx', false);
            }
        });
    },

    swapItems: function() {
        const selectedItemIndex = this.currentItemIndex;
        const targetIndex = this.targetItemIndex;

        console.log(`Before Swap: Selected Item Index: ${selectedItemIndex}, Target Item Index: ${targetIndex}`);
        console.log('Inventory:', this.inventory.map(item => item ? item.name : 'empty slot'));

        const selectedItem = this.inventory[selectedItemIndex];
        const targetItem = this.inventory[targetIndex];

        if (selectedItem && targetItem) {
            [this.inventory[selectedItemIndex], this.inventory[targetItemIndex]] = 
            [this.inventory[targetItemIndex], this.inventory[selectedItemIndex]];

            this.currentItemIndex = targetIndex;
        }

        console.log('After Swap:', this.inventory.map(item => item ? item.name : 'empty slot'));

        this.clearHighlights();
        this.isItemSelected = false;
        this.targetItemIndex = null;
    },

    highlightSelectedItem: function() {
        this.clearHighlights();
        const selectedItem = this.inventory[this.currentItemIndex];
        if (selectedItem) {
            const selectedItemElement = document.querySelector(`.ui_quick_item[data-item="${selectedItem.name}"]`);
            if (selectedItemElement) {
                selectedItemElement.classList.add('border-2', 'border-green-500');
            }
        } else {
            const itemElements = document.querySelectorAll('.ui_quick_item');
            const itemElement = itemElements[this.currentItemIndex];
            if (itemElement) {
                itemElement.classList.add('border-2', 'border-dashed', 'border-yellow-500');
            }
        }
    },

    selectItem: function(index) {
        this.clearHighlights();
        const itemElements = document.querySelectorAll('.ui_quick_item');
        const itemElement = itemElements[index];
        if (itemElement) {
            itemElement.classList.add('border-2', 'border-dashed', 'border-yellow-500');
        }
        const selectedItem = this.inventory[index];
        const sprite = game.sprites[game.playerid];
        if (selectedItem && sprite) {
            sprite.currentItem = selectedItem.name;
        } else if (sprite) {
            sprite.currentItem = null;
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
        for (let i = 0; i < 15; i++) {
            const item = this.inventory[i];
            const itemElement = document.createElement('div');
            itemElement.className = 'ui_quick_item relative cursor-move w-14 h-14 bg-[#18202f] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center';
            itemElement.dataset.item = item ? item.name : '';
            if (item) {
                const condition = 100 - (item.damage || 0);
                let barColor = 'bg-green-500';
                if (condition <= 15) {
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

    displayInventoryItems: function() {
        if (!game.itemsData || !game.itemsData.items) {
            console.error("itemsData or items array is not defined.");
            return;
        }
        this.inventory.forEach((item, index) => {
            if (!item) {
                return;
            }
            const itemData = game.itemsData.items.find(data => data.name === item.name);
            if (itemData) {
                let itemElement = document.querySelector(`.ui_quick_item[data-item="${item.name}"]`);
                if (itemElement) {
                    this.setItemIcon(itemElement, itemData);
                    itemElement.dataset.cd = itemData.cd;
                    itemElement.querySelector('.items_icon').classList.add(`items_${item.name}`);
                }
            } else {
                console.error('Item data not found for item:', item);
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

        // Play the drag sound effect when dragging starts
        audio.playAudio("dragStart", assets.load('dragStartSound'), 'sfx', false);  // Replace 'dragStartSound' with the actual sound file name

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
            e.stopPropagation();  // Stops some browsers from redirecting.
        }

        const target = e.target.closest('.ui_quick_item');
        if (this.dragSrcEl !== target && target) {
            const tempInnerHTML = this.dragSrcEl.innerHTML;
            const tempDataItem = this.dragSrcEl.dataset.item;

            if (tempDataItem && target.dataset.item) {
                this.dragSrcEl.innerHTML = target.innerHTML;
                target.innerHTML = tempInnerHTML;

                this.dragSrcEl.dataset.item = target.dataset.item;
                target.dataset.item = tempDataItem;

                const srcIndex = this.inventory.findIndex(item => item.name === tempDataItem);
                const targetIndex = this.inventory.findIndex(item => item.name === this.dragSrcEl.dataset.item);

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

                const srcIndex = this.inventory.findIndex(item => item.name === tempDataItem);
                if (srcIndex !== -1) {
                    this.inventory.splice(srcIndex, 1);  // Remove the item from the original position
                    this.inventory.push({ name: tempDataItem, amount: 1 });
                    this.currentItemIndex = this.inventory.length - 1;
                }
            }

            this.updateScale(this.dragSrcEl);
            this.updateScale(target);

            this.updateItemBadges();

            this.clearHighlights();
            this.selectItem(this.currentItemIndex);

            // Play drop sound effect
            audio.playAudio("sceneDrop", assets.load('sceneDrop'), 'sfx', false);

        } else {
            // Play slot drop sound effect
            audio.playAudio("slotDrop", assets.load('slotDrop'), 'sfx', false);
        }

        return false;
    },

    handleDragEnd: function(e) {
        const draggableItems = document.querySelectorAll('.ui_quick_item');
        draggableItems.forEach(item => {
            item.classList.remove('dragging');
            item.style.cursor = 'grab';
        });

        if (this.dragClone) {
            document.body.removeChild(this.dragClone);
            this.dragClone = null;
        }
    },

    updateItemBadges: function() {
        document.querySelectorAll('.ui_quick_item').forEach(item => {
            const itemName = item.dataset.item;
            const badge = item.querySelector('.item-badge');
            if (badge) {
                const inventoryItem = this.inventory.find(i => i.name === itemName);
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
    }
};

ui_inventory_window.start();

  </script>
</div>
<?php 
}
?>
