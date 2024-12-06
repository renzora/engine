const utils = {
    functionCalls: {},
    gameTime: {
        hours: 10,
        minutes: 0,
        seconds: 0,
        days: 0,
        speedMultiplier: 100,
        daysOfWeek: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        update: function(deltaTime) {
            if (!game.timeActive) return;  // Stop time updates if time is not active
            
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
        // Initialize tracking for the function if not already present
        if (!this.functionCalls[functionName]) {
            this.functionCalls[functionName] = {
                frameCount: 0, // Tracks how many times the function is executed in the current frame
                valueHistory: [], // Stores actual values passed in
                countHistory: [], // Stores per-frame call counts
                lastValue: null // Tracks the last value passed
            };
        }

        const trackedFunction = this.functionCalls[functionName];

        // Increment the count for this function in the current frame
        trackedFunction.frameCount++;

        // If a value is provided, update the lastValue and store it
        if (value !== null) {
            trackedFunction.lastValue = value;
        }
    },

    finalizeFrame: function() {
        // At the end of each frame, push frame counts and values into history and reset
        for (const key in this.functionCalls) {
            const trackedFunction = this.functionCalls[key];

            // Save the frame count and last value into their respective histories
            trackedFunction.countHistory.push(trackedFunction.frameCount);
            trackedFunction.valueHistory.push(trackedFunction.lastValue);

            // Reset frame count for the next frame
            trackedFunction.frameCount = 0;

            // Limit history lengths to avoid excessive memory usage
            if (trackedFunction.countHistory.length > game.maxFpsHistory) {
                trackedFunction.countHistory.shift();
            }
            if (trackedFunction.valueHistory.length > game.maxFpsHistory) {
                trackedFunction.valueHistory.shift();
            }
        }
    },

    getTrackedCalls: function() {
        // Returns both count and value histories for all tracked functions
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
    
    objExists: function(objName) {
        try {
            return typeof eval(objName) !== 'undefined';
        } catch (e) {
            return false;
        }
    },

    setZoomLevel: function(newZoomLevel) {
        const baseZoom = 4; // Default zoom level for larger viewports
        const minZoom = 1; // Minimum zoom level for very small viewports
        const maxZoom = 8; // Maximum zoom level for very large viewports
    
        // Calculate a scale factor based on the viewport dimensions
        const scaleFactor = Math.min(window.innerWidth / game.worldWidth, window.innerHeight / game.worldHeight);
    
        // Adjust the zoom level within the defined bounds
        game.zoomLevel = Math.max(minZoom, Math.min(maxZoom, Math.round(baseZoom * scaleFactor)));
    
        // Save zoom level to localStorage for persistence
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
    },

    fullScreen: function() {
        if (!document.fullscreenElement) {
          document.documentElement.requestFullscreen().catch((err) => {
            console.error(`Error attempting to enable fullscreen mode: ${err.message}`);
          });
        } else {
          document.exitFullscreen().catch((err) => {
            console.error(`Error attempting to exit fullscreen mode: ${err.message}`);
          });
        }
      }
}