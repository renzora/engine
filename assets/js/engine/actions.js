const actions = {

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

    checkForNearbyItems: function() {
        // Ensure roomData and items are present
        if (!game.roomData || !game.roomData.items) {
            return;
        }
    
        // Ensure mainSprite exists before proceeding
        const sprite = game.mainSprite;
        if (!sprite) {
            return;
        }
    
        // Define the sprite's boundary box
        const spriteBoundary = {
            left: sprite.x,
            right: sprite.x + sprite.width,
            top: sprite.y,
            bottom: sprite.y + sprite.height
        };
    
        let closestItem = null;
    
        // Loop through room items
        game.roomData.items.forEach(item => {
            const objectData = game.objectData[item.id];
            if (!objectData || !objectData[0] || !objectData[0].script) return;
    
            const script = objectData[0].script;
    
            // Calculate the object's full width and height
            const itemX = Math.min(...item.x) * 16; // Leftmost x-coordinate of the object (in pixels)
            const itemY = Math.min(...item.y) * 16; // Topmost y-coordinate of the object (in pixels)
            const itemWidth = (Math.max(...item.x) - Math.min(...item.x) + 1) * 16; // Full width of the object (in pixels)
            const itemHeight = (Math.max(...item.y) - Math.min(...item.y) + 1) * 16; // Full height of the object (in pixels)
    
            // Define the object's boundary box
            const objectBoundary = {
                left: itemX,
                right: itemX + itemWidth,
                top: itemY,
                bottom: itemY + itemHeight
            };
    
            // Check if the sprite is overlapping with the object
            const isSpriteInsideObject = (
                spriteBoundary.right >= objectBoundary.left &&
                spriteBoundary.left <= objectBoundary.right &&
                spriteBoundary.bottom >= objectBoundary.top &&
                spriteBoundary.top <= objectBoundary.bottom
            );
    
            // If the sprite is inside the object's boundary, handle actions
            if (isSpriteInsideObject) {
                closestItem = item;
    
                const spriteScreenX = (sprite.x - camera.cameraX) * game.zoomLevel;
                const spriteScreenY = (sprite.y - camera.cameraY) * game.zoomLevel;
    
                const canvasRect = game.canvas.getBoundingClientRect();
                const tooltipX = canvasRect.left + spriteScreenX;
                const tooltipY = canvasRect.top + spriteScreenY - sprite.height - 10; // Adjust -10 for spacing above the sprite
    
                // Show tooltip if defined in the script
                if (script.walk && script.walk.tooltip) {
                    this.showTooltip(script.walk.tooltip, tooltipX, tooltipY);
                }
    
                // Handle walk actions like sway or pick up
                if (script.walk && script.walk.sway) {
                    // Trigger the sway effect for the item
                    closestItem.isRotating = true;
                    closestItem.rotationElapsed = 0;
                    actions.handleRotation(closestItem);
                }
    
                // Handle picking up items with gamepadY
                if (script.walk && script.walk.gamepadY && input.isYButtonHeld) {
                    const reward = script.walk.gamepadY.reward;
                    if (reward) {
                        // Add item to inventory using ui_inventory_window.addToInventory
                        ui_inventory_window.addToInventory(reward.id, reward.amount); // Using the existing inventory function from ui_inventory_window
                    }
                    this.hideTooltip();
                }
    
                // Handle scene loading if the script includes a scene action
                if (script.walk && script.walk.scene) {
                    if (input.isAButtonHeld) {  // Check if the "A" button is held to trigger the scene change
                        game.loadScene(script.walk.scene);  // Load the new scene
                        this.hideTooltip();  // Hide the tooltip after loading the scene
                    }
                }
    
                // Handle item dropping logic
                if (script.drop) {
                    const selectedItem = ui_inventory_window.selectedInventoryItem;  // Get the currently selected item from inventory
                    if (selectedItem && selectedItem.name === script.drop.requiredItem) {
                        // Show drop action tooltip
                        this.showTooltip(`Press [X] to drop ${selectedItem.name}`, tooltipX, tooltipY);
    
                        // Check if the player presses the "X" button to drop the item
                        if (input.isXButtonHeld) {
                            const actionFunction = script.drop.action;
    
                            if (typeof actions[actionFunction] === 'function') {
                                // Execute the action function
                                actions[actionFunction](selectedItem, closestItem);
                            } else {
                                console.error(`Action function ${actionFunction} not found in actions object`);
                            }
    
                            this.hideTooltip();  // Hide tooltip after performing the drop action
                        }
                    }
                }
            }
        });
    
        // If no item is nearby or the sprite is outside of any object, hide the tooltip
        if (!closestItem) {
            this.hideTooltip();
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