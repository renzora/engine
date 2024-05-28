// actions.js
const actions = {
    chopTree: function(x, y) {
        game.utils.addItem('log', { x: x, y: y });
        game.utils.removeItem('tree', x / 16, y / 16);
        console.log("Tree chopped down at", x, y);
    },

    openDoor: function(x, y) {
        console.log("Door opened at", x, y);
        // Add logic to open the door
    },

    pickUp: function(x, y) {
        console.log("Item picked up at", x, y);
        // Add logic to pick up the item
    },

    drop: function(x, y) {
        console.log("Item dropped at", x, y);
        // Add logic to drop the item
    },

    push: function(x, y) {
        console.log("Item pushed at", x, y);
        // Add logic to push the item
    },

    jump: function(x, y) {
        console.log("Jump at", x, y);
        // Add logic to jump
    },

    sit: function(x, y) {
        console.log("Sit at", x, y);
        // Add logic to sit
    },

    cook: function(x, y) {
        console.log("Cooking at", x, y);
        // Add logic to cook
    },

    water: function(x, y) {
        console.log("Watering at", x, y);
        // Add logic to water plants
    },

    dig: function(x, y) {
        console.log("Digging at", x, y);
        // Add logic to dig
    },

    performAction: function(result, x, y) {
        switch(result) {
            case "log":
                this.chopTree(x, y);
                break;
            case "door_open":
                this.openDoor(x, y);
                break;
            // Add more case handlers as needed
            default:
                console.log("Unknown action result:", result);
        }
    },

    handleObjectInteraction: function(x, y) {
        if (game.roomData && game.roomData.items) {
            for (const roomItem of game.roomData.items) {
                const itemScript = assets.load('objectScript')[roomItem.id];
                if (!itemScript) continue;

                const xCoordinates = roomItem.x || [];
                const yCoordinates = roomItem.y || [];

                for (let i = 0; i < xCoordinates.length; i++) {
                    const itemX = parseInt(xCoordinates[i], 10) * 16;
                    const itemY = parseInt(yCoordinates[i], 10) * 16;

                    if (x >= itemX && x <= itemX + 16 && y >= itemY && y <= itemY + 16) {
                        if (itemScript.actions) {
                            for (const action in itemScript.actions) {
                                // Here you could check for specific conditions, such as required items
                                if (action === 'chop' && this.isAxeEquipped()) {
                                    // Perform the action
                                    const result = itemScript.actions[action].result;
                                    console.log(`Action performed: ${action}, Result: ${result}`);
                                    this.performAction(result, itemX, itemY);
                                }
                                // Add more action conditions here
                            }
                        }
                    }
                }
            }
        }
    },

    handleWalkOnTile: function(x, y) {
        if (game.roomData && game.roomData.items) {
            for (const roomItem of game.roomData.items) {
                const itemScript = assets.load('objectScript')[roomItem.id];
                if (!itemScript) continue;

                const xCoordinates = roomItem.x || [];
                const yCoordinates = roomItem.y || [];

                for (let i = 0; i < xCoordinates.length; i++) {
                    const itemX = parseInt(xCoordinates[i], 10) * 16;
                    const itemY = parseInt(yCoordinates[i], 10) * 16;

                    if (x === itemX && y === itemY) {
                        if (itemScript.triggers && itemScript.triggers.walk) {
                            const result = itemScript.triggers.walk.result;
                            console.log(`Walk trigger activated: Result: ${result}`);
                            this.performAction(result, itemX, itemY);
                        }
                    }
                }
            }
        }
    },

    isAxeEquipped: function() {
        const mainSprite = game.sprites['main'];
        return mainSprite && mainSprite.currentItem === 'axe';
    }
}
