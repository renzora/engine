const actions = {
    objectScript: {},

    loadObjectScript: function () {
        this.objectScript = assets.load('objectScript');
        console.log('Object script loaded:', this.objectScript);
    },

    handleTileAction: function (tileId) {
        const tileActions = this.objectScript[tileId];
        if (tileActions && tileActions.walk) {
            if (tileActions.walk.swim) {
                this.swim();
            }
            if (tileActions.walk.tooltip) {
                console.log("tooltip executed");
                this.addTooltip(tileActions.walk.tooltip, game.mainSprite.x, game.mainSprite.y);
            }
        }
    },

    handleExitTileAction: function (tileId) {
        const tileActions = this.objectScript[tileId];
        if (tileActions && tileActions.exit) {
            if (tileActions.exit.swim === false) {
                this.restoreBody();
            }
            if (tileActions.exit.tooltip) {
                this.addTooltip(tileActions.exit.tooltip, game.mainSprite.x, game.mainSprite.y);
            }
        }
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
        console.log('dropItemOnObject called with:', item, object);
        const objectActions = this.objectScript[object.id];
        console.log('Object actions:', objectActions);
        if (objectActions && objectActions.drop) {
            const dropAction = objectActions.drop;
            if (dropAction.requiredItem === item) {
                const actionFunction = dropAction.action;
                console.log('Executing action function:', actionFunction);
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
        this.addTooltip(`Invalid drop of ${item} on ${objectId}`, game.mainSprite.x, game.mainSprite.y);
    },

    getObjectNameById: function(objectId) {
        return objectId;
    },
    
    dropWoodOnCampfire: function (item, object) {
        console.log('YAYY WOOD!');
    },

    checkForNearbyItems: function() {
        if (!game.roomData || !game.roomData.items) {
            return;
        }
    
        const maxPickupRadius = 16; // Define a maximum radius within which the sprite can pick up items
        const headroom = 8; // Define the headroom around the boundary
        const sprite = game.mainSprite;
    
        // Round sprite position to nearest integer
        const spriteX = Math.round(sprite.x);
        const spriteY = Math.round(sprite.y);
    
        let closestItem = null;
    
        game.roomData.items.forEach(item => {
            const itemData = game.objectData[item.id];
            if (!itemData || itemData.length === 0) return;
    
            const xCoordinates = item.x.map(x => x * 16); // Convert to pixel coordinates
            const yCoordinates = item.y.map(y => y * 16); // Convert to pixel coordinates
    
            // Calculate the bounding box of the item
            const minX = Math.min(...xCoordinates) - headroom;
            const maxX = Math.max(...xCoordinates) + headroom;
            const minY = Math.min(...yCoordinates) - headroom;
            const maxY = Math.max(...yCoordinates) + headroom;
    
            // Check if the sprite is inside the item's boundary
            if (spriteX >= minX && spriteX <= maxX && spriteY >= minY && spriteY <= maxY) {
                closestItem = item;
            }
        });
    
        if (closestItem) {
            const itemData = game.objectData[closestItem.id][0]; // Access the first element for item details
            const itemName = itemData.n; // Get the item name
            const itemX = closestItem.x[0] * 16;
            const itemY = closestItem.y[closestItem.y.length - 1] * 16; // Use the last Y coordinate for the bottom tile
            this.addTooltip(`[X] pickup ${itemName}`, itemX, itemY);
        }
    },    

    // Tooltip methods
    addTooltip: function(message, x, y) {
        game.tooltips.push({ message, x, y });
    },
};
