utils = {
    functionCalls: {},
    functionQueue: [],
    gameTime: {
        hours: 22,
        minutes: 0,
        seconds: 0,
        days: 0,
        speedMultiplier: 100,
        daysOfWeek: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        update: function(deltaTime) {
            if (!game.timeActive) return;
            
            const gameSeconds = (deltaTime / 1000) * this.speedMultiplier;
            this.seconds += gameSeconds;
    
            if (this.seconds >= 60) {
                this.minutes += Math.floor(this.seconds / 60);
                this.seconds = this.seconds % 60;
            }
            if (this.minutes >= 60) {
                this.hours += Math.floor(this.minutes / 60);
                this.minutes = this.minutes % 60;
            }
            if (this.hours >= 24) {
                this.days += Math.floor(this.hours / 24);
                this.hours = this.hours % 24;
            }
        },
        display: function() {
            const pad = (num) => String(num).padStart(2, '0');
            const dayOfWeek = this.daysOfWeek[this.days % 7];
            return `${dayOfWeek} ${pad(this.hours)}:${pad(this.minutes)}`;
        }
    },

    tracker: function(functionName, value = null) {
        if (!this.functionCalls[functionName]) {
            this.functionCalls[functionName] = {
                frameCount: 0,
                valueHistory: [],
                countHistory: [],
                lastValue: null
            };
        }

        const trackedFunction = this.functionCalls[functionName];

        trackedFunction.frameCount++;

        if (value !== null) {
            trackedFunction.lastValue = value;
        }
    },

    finalizeFrame: function() {
        for (const key in this.functionCalls) {
            const trackedFunction = this.functionCalls[key];

            trackedFunction.countHistory.push(trackedFunction.frameCount);
            trackedFunction.valueHistory.push(trackedFunction.lastValue);
            trackedFunction.frameCount = 0;

            if (trackedFunction.countHistory.length > game.maxFpsHistory) {
                trackedFunction.countHistory.shift();
            }
            if (trackedFunction.valueHistory.length > game.maxFpsHistory) {
                trackedFunction.valueHistory.shift();
            }
        }
    },

    getTrackedCalls: function() {
        const result = {};
        for (const key in this.functionCalls) {
            result[key] = {
                countHistory: this.functionCalls[key].countHistory,
                valueHistory: this.functionCalls[key].valueHistory
            };
        }
        return result;
    },

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

    setZoomLevel: function(newZoomLevel) {
        localStorage.setItem('zoomLevel', game.zoomLevel);
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
            const cleanLine = line.split('#')[0].trim();
    
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
    },

    generateId: function () {
        const timestamp = Date.now().toString(36).slice(-4);
        const randomPart = Math.random().toString(36).substring(2, 5);
        return `${timestamp}${randomPart}`;
    }
    
}