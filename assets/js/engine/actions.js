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
        }
    },

    handleExitTileAction: function (tileId) {
        const tileActions = this.objectScript[tileId];
        if (tileActions && tileActions.exit) {
            if (tileActions.exit.swim === false) {
                this.restoreBody();
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
    },

    getObjectNameById: function(objectId) {
        return objectId;
    },
    
    dropWoodOnCampfire: function (item, object) {
        console.log('YAYY WOOD!');
    },
};
