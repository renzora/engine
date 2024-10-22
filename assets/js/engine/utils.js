const utils = {
    getTileIdAt: function(x, y) {
        if (!game.roomData || !game.roomData.items) {
            return null;
        }
    
        for (const item of game.roomData.items) {
            const xCoordinates = item.x || [];
            const yCoordinates = item.y || [];
    
            if (xCoordinates.includes(x) && yCoordinates.includes(y)) {
                return item.id;
            }
        }
        return null;
    },

    findObjectAt: function(x, y) {
        if (!game.roomData || !game.roomData.items) return null;

        const renderQueue = [];

        game.roomData.items.forEach(roomItem => {
            const itemData = assets.use('objectData')[roomItem.id];
            if (itemData && itemData.length > 0) {
                const tileData = itemData[0];
                const xCoordinates = roomItem.x || [];
                const yCoordinates = roomItem.y || [];

                let index = 0;

                for (let tileY = Math.min(...yCoordinates); tileY <= Math.max(...yCoordinates); tileY++) {
                    for (let tileX = Math.min(...xCoordinates); tileX <= Math.max(...xCoordinates); tileX++) {
                        const posX = tileX * 16;
                        const posY = tileY * 16;

                        let tileFrameIndex;
                        if (tileData.d) {
                            const currentFrame = tileData.currentFrame || 0;
                            tileFrameIndex = Array.isArray(tileData.i) ? tileData.i[(currentFrame + index) % tileData.i.length] : tileData.i;
                        } else {
                            tileFrameIndex = tileData.i[index];
                        }

                        renderQueue.push({
                            tileIndex: tileFrameIndex,
                            posX: posX,
                            posY: posY,
                            z: Array.isArray(tileData.z) ? tileData.z[index % tileData.z.length] : tileData.z,
                            id: roomItem.id,
                            item: roomItem
                        });

                        index++;
                    }
                }
            }
        });

        renderQueue.sort((a, b) => a.z - b.z || a.renderOrder - b.renderOrder);

        let highestZIndexObject = null;

        for (const item of renderQueue) {
            const tileRect = {
                x: item.posX,
                y: item.posY,
                width: 16,
                height: 16
            };

            if (
                x >= tileRect.x &&
                x <= tileRect.x + tileRect.width &&
                y >= tileRect.y &&
                y <= tileRect.y + tileRect.height
            ) {
                highestZIndexObject = item.item;
            }
        }

        return highestZIndexObject;
    },
    
    objExists: function(objName) {
        try {
            return typeof eval(objName) !== 'undefined';
        } catch (e) {
            return false;
        }
    },

    parseYaml: function(yaml) {
        const lines = yaml.split('\n');
        const result = {};
        let currentObject = result;
        let objectStack = [result];
        let indentStack = [0];
        let previousIndent = 0;
        let lastKey = '';
    
        lines.forEach(line => {
            // Remove comments after #
            const cleanLine = line.split('#')[0].trim();
    
            // Skip empty lines after removing comments
            if (cleanLine === '') return;
    
            const indent = line.search(/\S/);
    
            if (indent < previousIndent && objectStack.length > 1) {
                while (indent <= indentStack[indentStack.length - 1]) {
                    objectStack.pop();
                    indentStack.pop();
                }
                currentObject = objectStack[objectStack.length - 1];
            }
    
            if (cleanLine.startsWith('- ')) {
                const listItem = cleanLine.slice(2).trim().replace(/^["']|["']$/g, '');
                if (Array.isArray(currentObject[lastKey])) {
                    currentObject[lastKey].push(listItem);
                } else {
                    currentObject[lastKey] = [listItem];
                }
            } else if (cleanLine.includes(':')) {
                const [rawKey, ...rawValue] = cleanLine.split(':');
                const key = rawKey.trim();
                let value = rawValue.join(':').trim().replace(/^["']|["']$/g, '');
    
                if (value === '') {
                    currentObject[key] = {};
                    objectStack.push(currentObject[key]);
                    currentObject = currentObject[key];
                    indentStack.push(indent);
                } else {
                    currentObject[key] = value;
                }
                lastKey = key;
            }
    
            previousIndent = indent;
        });
    
        return result;
    }    
    
}