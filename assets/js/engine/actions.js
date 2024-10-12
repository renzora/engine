const actions = {
    handleTileAction: function(tileId) {
        const tileData = game.objectData[tileId];
        if (!tileData || !tileData[0]) return;
    
        const tileScript = tileData[0].script;
        const sprite = game.mainSprite;
        const spriteX = sprite.x;  // Use exact sprite coordinates (not rounded)
        const spriteY = sprite.y;
    
        const proximityThreshold = 16;  // Define a proximity threshold (16 pixels)
    
        if (tileScript && tileScript.walk) {
            if (tileScript.walk.requiredButton && this.isButtonPressed(tileScript.walk.requiredButton)) {

            }
    
            // Handle grass cutting if the "cut" action is defined
            if (tileScript.walk.cut === true) {
                this.chop();  // Call the chop function for cutting grass
                console.log('Grass cut action triggered!');
            }
    
            if (tileScript.walk.swim) {
                this.swim();
            }
    
            if (tileScript.walk.sway === true) {
                // Find the specific object in roomData near the sprite's current position
                game.roomData.items.forEach(roomItem => {
                    const xCoordinates = roomItem.x || [];
                    const yCoordinates = roomItem.y || [];
    
                    // Adjust the logic to account for all tiles occupied by the object
                    const isNearObject = xCoordinates.some((x, i) => {
                        const y = yCoordinates[i];
    
                        // Check if the player is within proximity to any tile of the object
                        return Math.abs(x * 16 - spriteX) <= proximityThreshold &&
                               Math.abs(y * 16 - spriteY) <= proximityThreshold;
                    });
    
                    // Ensure the correct tile is rotating and it's near the player
                    if (roomItem.id === tileId && isNearObject) {
                        // Start rotation for the object
                        roomItem.isRotating = true;
                        roomItem.rotationElapsed = 0;  // Initialize rotationElapsed for smooth tracking
                        actions.handleRotation(roomItem);
                    }
                });
            }
        }
    },    


    handleExitTileAction: function(tileId) {
        const tileData = game.objectData[tileId];
        if (!tileData || !tileData[0] || !tileData[0].script) return;

        const tileScript = tileData[0].script;

        if (tileScript.exit) {
            if (tileScript.exit.swim === false) {
                this.restoreBody();
            }
        }
    },

    handleRotation: function(roomItem) {
        let baseSwayAngle = Math.PI / 12; // Default baseline sway angle
        let directionMultiplier = 1; // Multiplier to affect the sway direction and intensity

        const sprite = game.sprites[game.playerid]; // Assuming the player sprite controls direction
        if (sprite) {
            if (sprite.direction === 'left' || sprite.direction === 'W') {
                directionMultiplier = -1; // Sway more to the left
            } else if (sprite.direction === 'right' || sprite.direction === 'E') {
                directionMultiplier = 1; // Sway more to the right
            }

            // Calculate sway using time and direction
            const maxSwayAngle = baseSwayAngle + (Math.random() * Math.PI / 24) * directionMultiplier;
            const swaySpeed = 150;
            const totalRotationDuration = 150;
            const recoveryTime = 300;
            const elapsedTime = roomItem.rotationElapsed || 0;
            roomItem.rotationElapsed = elapsedTime + game.deltaTime;

            let sway = 0;
            if (elapsedTime < totalRotationDuration) {
                sway = directionMultiplier * (Math.sin((elapsedTime / totalRotationDuration) * (Math.PI / 2)) * maxSwayAngle);
            } else if (elapsedTime < totalRotationDuration + recoveryTime) {
                const recoveryElapsed = elapsedTime - totalRotationDuration;
                sway = directionMultiplier * (Math.cos((recoveryElapsed / recoveryTime) * (Math.PI / 2)) * maxSwayAngle);
            }

            roomItem.rotation = sway;

            // Reset rotation after a complete cycle
            if (elapsedTime >= totalRotationDuration + recoveryTime) {
                roomItem.isRotating = false;
                roomItem.rotationElapsed = 0;
                roomItem.rotation = 0;
            }
        }
    },

    getCategory: function(itemName) {
        const categories = {
            sword: "pewpew",
            potion: "meals",
            shield: "defence",
            key: "random",
            wood: "pewpew",
            gift: "random",
            fireball: "pewpew",
            banana: "meals",
            sweet: "meals",
            fish: "meals"
        };
        return categories[itemName] || "random";
    },

    swim: function () {
        game.sprites[game.playerid].body = 0;
        game.sprites[game.playerid].speed = 55;
    },

    restoreBody: function () {
        game.sprites[game.playerid].body = 1;
        game.sprites[game.playerid].speed = 90;
    },

    chop: function () {
        console.log("Chopping tree!");
    },
    
    dropItemOnObject: function (item, object) {
        const objectData = game.objectData[object.id];
        if (!objectData || !objectData[0]) {
            console.log(`No data found for object ${object.id}`);
            return;
        }

        const objectActions = objectData[0].script;
        if (objectActions && objectActions.drop) {
            const dropAction = objectActions.drop;

            // Check if the required button is pressed for the drop action
            if (dropAction.requiredButton && !this.isButtonPressed(dropAction.requiredButton)) {
                console.log(`Action requires button ${dropAction.requiredButton} to be pressed.`);
                return; // Exit if the required button is not pressed
            }

            if (dropAction.requiredItem === item) {
                const actionFunction = dropAction.action;
                if (typeof this[actionFunction] === 'function') {
                    this[actionFunction](item, object);
                } else {
                    console.error(`Action function ${actionFunction} not found in actions object`);
                }
            } else {
                this.notifyInvalidDrop(item, object.id);
            }
        } else {
            console.log(`No drop action defined for ${object.id}`);
        }
    },    

    notifyInvalidDrop: function(item, objectId) {
        const objectName = this.getObjectNameById(objectId);
        ui.notif("scene_change_notif", `You cannot drop a ${item} into ${objectName}`, true);
    },

    getObjectNameById: function(objectId) {
        return objectId;
    },
    
    dropWoodOnCampfire: function (item, object) {
        console.log('YAYY WOOD!');
    },

    addToInventory: function(itemName) {
        // Ensure that ui_inventory_window and its inventory array are initialized
        if (!ui_inventory_window || !Array.isArray(ui_inventory_window.inventory)) {
            console.error("ui_inventory_window or its inventory array is not initialized.");
            return;
        }
        
        // Look up the item in the itemsData to check its properties
        const itemData = game.itemsData.items.find(data => data.name === itemName);
        
        if (!itemData) {
            console.error(`Item data for ${itemName} not found.`);
            return;
        }
        
        // Check if the item is already in the inventory and has collect set to false
        const inventoryItem = ui_inventory_window.inventory.find(item => item.name === itemName);
        
        if (inventoryItem && itemData.collect === false) {
            console.log(`${itemName} is already in the inventory and cannot be collected again.`);
            return; // Exit if the item cannot be collected again and is already in the inventory
        }
        
        // Track the currently selected index before adding the new item
        const previousSelectedIndex = ui_inventory_window.currentItemIndex;
        
        // Remove the item from roomData only if it's being collected for the first time or if it can be collected multiple times
        if (!inventoryItem || itemData.collect !== false) {
            const itemIndex = game.roomData.items.findIndex(item => {
                const itemRoomData = game.objectData[item.id];
                return itemRoomData && itemRoomData[0].n === itemName;
            });
            
            if (itemIndex !== -1) {
                game.roomData.items.splice(itemIndex, 1);
            }
        }
        
        // Set the collected flag to true for this item if it has collect set to false
        if (itemData.collect === false) {
            itemData.collected = true;
        }
        
        // Add the item to the currently active tab
        const currentTab = ui_inventory_window.currentTab;
        
        // Check if the item is already in the current tab
        const tabItem = ui_inventory_window.inventory.find(item => item.name === itemName && item.category === currentTab);
        
        if (tabItem) {
            tabItem.amount += 1; // Increase the amount if it already exists in the active tab
        } else {
            // Add new item to the current tab in the inventory if not already present
            ui_inventory_window.inventory.push({
                name: itemName,
                amount: 1,
                category: currentTab,
                damage: 0
            });
        }
        
        // Re-render the inventory items
        ui_inventory_window.renderInventoryItems();
        ui_inventory_window.displayInventoryItems();
        ui_inventory_window.updateItemBadges();
        
        // Reapply the selection after re-rendering
        if (previousSelectedIndex !== null && previousSelectedIndex < ui_inventory_window.inventory.length) {
            ui_inventory_window.selectItem(previousSelectedIndex);
        }
    },

    checkForNearbyItems: function() {
        // Ensure roomData and items are present
        if (!game.roomData || !game.roomData.items) {
            return;
        }
        
        // Ensure mainSprite exists before proceeding
        const sprite = game.mainSprite;
        if (!sprite) {
            //console.warn('Main sprite is not available.');
            return;
        }
        
        const maxPickupRadius = 16;
        const spriteX = Math.round(sprite.x);
        const spriteY = Math.round(sprite.y);
        
        let closestItem = null;
        let closestDistance = maxPickupRadius;
        
        // Loop through room items
        game.roomData.items.forEach(item => {
            const objectData = game.objectData[item.id];
            if (!objectData || objectData.length === 0) return;
        
            if (objectData[0].type !== "item") return;
        
            const itemX = item.x[0] * 16;
            const itemY = item.y[0] * 16; 
            const distance = Math.sqrt(Math.pow(spriteX - itemX, 2) + Math.pow(spriteY - itemY, 2));
    
            if (distance <= maxPickupRadius && distance < closestDistance) {
                closestItem = item;
                closestDistance = distance;
            }
        });
        
        // If a close item is found
        if (closestItem) {
            const objectData = game.objectData[closestItem.id][0];
            const itemName = objectData.n;
            const itemData = game.itemsData.items.find(data => data.name === itemName);
    
            if (!itemData) {
                console.error(`Item data for ${itemName} not found in itemsData.`);
                return;
            }
    
            const itemScreenX = (closestItem.x[0] * 16 - camera.cameraX) * game.zoomLevel;
            const itemScreenY = (closestItem.y[0] * 16 - camera.cameraY) * game.zoomLevel;
            const canvasRect = game.canvas.getBoundingClientRect();
            const tooltipX = canvasRect.left + itemScreenX;
            const tooltipY = canvasRect.top + itemScreenY;
    
            const inventoryItem = ui_inventory_window.inventory.find(item => item.name === itemName);
            if (inventoryItem && itemData.collect === false) {
                console.log(`Cannot pick up item: ${itemName} already in inventory`);
                this.showTooltip(`${itemName} Already in inventory`, tooltipX, tooltipY);
                return;
            }
    
            this.showTooltip(`Press Y to pick up ${itemName}`, tooltipX, tooltipY);
    
            // Check if the "Y" button is held
            if (input.isYButtonHeld) {
                audio.playAudio('itemPickup', assets.load('itemEquip'), 'sfx');
    
                // Ensure the item is not in the inventory before adding
                if (!(ui_inventory_window.inventory.find(item => item.name === itemName) && itemData.collect === false)) {
                    // Remove item from room data
                    game.roomData.items = game.roomData.items.filter(item => {
                        const itemX = item.x[0] * 16;
                        const itemY = item.y[0] * 16;
                        return !(itemX === closestItem.x[0] * 16 && itemY === closestItem.y[0] * 16 && item.id === closestItem.id);
                    });
                }
    
                // Add the item to inventory
                this.addToInventory(itemName);
    
                // Hide tooltip after picking up the item
                this.hideTooltip();
            }
        } else {
            this.hideTooltip();  // Hide tooltip if no item is nearby
        }
    },    

    // Utility to check if a required button is pressed
    isButtonPressed: function(button) {
        return gamepad.buttons.includes(button);
    },

    showTooltip: function(text, x, y) {
        let tooltip = document.getElementById('game_tooltip');
        if (!tooltip) {
            tooltip = document.createElement('div');
            tooltip.id = 'game_tooltip';
            tooltip.style.position = 'absolute';
            tooltip.style.padding = '5px';
            tooltip.style.backgroundColor = 'rgba(0, 0, 0, 0.7)';
            tooltip.style.color = 'white';
            tooltip.style.borderRadius = '5px';
            tooltip.style.pointerEvents = 'none';
            tooltip.style.zIndex = '1000'; // Ensure it's on top of other elements
            tooltip.style.whiteSpace = 'nowrap'; // Prevent text wrapping
            document.body.appendChild(tooltip);
        }
        
        tooltip.innerText = text;
        tooltip.style.display = 'block';
        
        // Calculate the width of the tooltip
        const tooltipWidth = tooltip.offsetWidth;
    
        // Center the tooltip horizontally
        tooltip.style.left = `${x - (tooltipWidth / 2)}px`;
        tooltip.style.top = `${y - 20}px`; // Position the tooltip above the item
    },
    
    hideTooltip: function() {
        const tooltip = document.getElementById('game_tooltip');
        if (tooltip) {
            tooltip.style.display = 'none';
        }
    }
};
