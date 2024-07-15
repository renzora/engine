var editor = {
    tileSize: 16,
    tilesPerRow: 150,
    fixedHeight: 32,
    selectedItem: null,
    activeItemGroup: null,
    offsetX: null,
    offsetY: null,
    clickHandler: null,
    mouseMoveHandler: null,
    mouseUpHandler: null,
    isPlacingItem: false,

    setupClickToActivate: function() {
        this.clickHandler = this.handleClick.bind(this);
        this.mouseMoveHandler = this.handleMouseMove.bind(this);
        this.mouseUpHandler = this.handleMouseUp.bind(this);

        document.querySelectorAll('.inventory-item').forEach(item => {
            item.addEventListener('click', this.clickHandler);
        });

        document.addEventListener('mousemove', this.mouseMoveHandler);
        document.addEventListener('mouseup', this.mouseUpHandler);
        document.addEventListener('contextmenu', function(event) {
            event.preventDefault();
        });

        game.isEditorActive = false; // Set the flag to indicate the editor is active
        console.log('Editor setupClickToActivate triggered');
    },

    teardownClickToActivate: function() {
        document.querySelectorAll('.inventory-item').forEach(item => {
            item.removeEventListener('click', this.clickHandler);
        });

        document.removeEventListener('mousemove', this.mouseMoveHandler);
        document.removeEventListener('mouseup', this.mouseUpHandler);
        game.pathfinding = true;
        modal.show('ui_window');
        modal.show('quick_menu_window');
        game.isEditorActive = false; // Set the flag to indicate the editor is no longer active
        console.log('Editor teardownClickToActivate triggered');
    },

    handleClick: function(event) {
        console.log('Editor handleClick triggered');
        if (event.target.closest('.tabs')) {
            return;
        }

        const originalItem = event.currentTarget;
        const originalCanvas = originalItem.querySelector('canvas');

        if (this.activeItemGroup) {
            this.activeItemGroup.classList.remove('active');
        }

        if (this.selectedItem) {
            this.selectedItem.remove();
        }

        this.selectedItem = document.createElement('div');
        this.selectedItem.classList.add('inventory-item-clone');
        this.selectedItem.style.position = 'absolute';
        this.selectedItem.style.pointerEvents = 'none';
        this.selectedItem.style.zIndex = 1000;
        this.selectedItem.dataset.category = originalItem.dataset.category;

        const clonedCanvas = document.createElement('canvas');
        clonedCanvas.width = originalCanvas.width;
        clonedCanvas.height = originalCanvas.height;
        clonedCanvas.getContext('2d').drawImage(originalCanvas, 0, 0);

        this.selectedItem.appendChild(clonedCanvas);
        document.body.appendChild(this.selectedItem);

        this.offsetX = originalCanvas.width / 2;
        this.offsetY = originalCanvas.height / 2;

        this.activeItemGroup = originalItem.closest('.inventory-item-group');
        this.activeItemGroup.classList.add('active');

        this.moveSelectedItem(event);
        game.pathfinding = false;
        this.isPlacingItem = true;
        //modal.hide('ui_window');
        //modal.hide('quick_menu_window');
    },

    handleMouseMove: function(event) {
        if (this.selectedItem) {
            const uiMenu = document.querySelector('[data-window="ui_window"]');
            if (uiMenu && uiMenu.contains(event.target)) {
                this.selectedItem.style.display = 'none';
            } else {
                this.selectedItem.style.display = 'block';
                this.moveSelectedItem(event);

                const zoomLevel = game.zoomLevel;
                const cameraX = game.cameraX;
                const cameraY = game.cameraY;

                const dropX = (event.clientX - this.offsetX + window.scrollX) / zoomLevel + cameraX;
                const dropY = (event.clientY - this.offsetY + window.scrollY) / zoomLevel + cameraY;

                const snappedX = Math.round(dropX / this.tileSize);
                const snappedY = Math.round(dropY / this.tileSize);

                const newItem = {
                    id: this.selectedItem.dataset.category,
                    x: [],
                    y: []
                };

                this.calculateTilePositions(this.selectedItem, snappedX, snappedY, this.tileSize, newItem.x, newItem.y);
                this.highlightOverlappingTiles(newItem);
            }
        }
    },

    handleMouseUp: function(event) {
        const isInWindow = event.target.closest('.window') !== null;

        if (this.selectedItem && !isInWindow && event.button === 0) {
            const zoomLevel = game.zoomLevel;
            const cameraX = game.cameraX;
            const cameraY = game.cameraY;

            const dropX = (event.clientX - this.offsetX + window.scrollX) / zoomLevel + cameraX;
            const dropY = (event.clientY - this.offsetY + window.scrollY) / zoomLevel + cameraY;

            const snappedX = Math.round(dropX / this.tileSize);
            const snappedY = Math.round(dropY / this.tileSize);

            const newItem = {
                id: this.selectedItem.dataset.category,
                x: [],
                y: []
            };

            if (event.shiftKey) {
                this.addItemsInLine(newItem, snappedX, snappedY, this.tileSize);
            } else {
                this.calculateTilePositions(this.selectedItem, snappedX, snappedY, this.tileSize, newItem.x, newItem.y);
                this.addNewItemToRoomData(newItem);
            }
        } else if (event.button === 2) {
            if (this.activeItemGroup) {
                this.activeItemGroup.classList.remove('active');
            }
            if (this.selectedItem) {
                this.selectedItem.remove();
                this.selectedItem = null;
                game.pathfinding = true;
                this.isPlacingItem = false;
                //modal.show('quick_menu_window');
                //modal.show('ui_window');
                game.overlappingTiles = [];
            }
        }
    },

    moveSelectedItem: function(event) {
        const zoomLevel = game.zoomLevel;
        const cameraX = game.cameraX;
        const cameraY = game.cameraY;

        const canvasX = (event.clientX - this.offsetX + window.scrollX) / zoomLevel + cameraX;
        const canvasY = (event.clientY - this.offsetY + window.scrollY) / zoomLevel + cameraY;

        const snappedX = Math.round(canvasX / this.tileSize) * this.tileSize;
        const snappedY = Math.round(canvasY / this.tileSize) * this.tileSize;

        this.selectedItem.style.left = `${(snappedX - cameraX) * zoomLevel + window.scrollX}px`;
        this.selectedItem.style.top = `${(snappedY - cameraY) * zoomLevel + window.scrollY}px`;

        this.selectedItem.style.transform = `scale(${zoomLevel})`;
        this.selectedItem.style.transformOrigin = 'top left';
    },

    calculateTilePositions: function(item, baseX, baseY, tileSize, xArray, yArray) {
        const canvas = item.querySelector('canvas');
        const width = canvas.width;
        const height = canvas.height;
        const cols = width / tileSize;
        const rows = height / tileSize;

        for (let col = 0; col < cols; col++) {
            xArray.push(baseX + col);
        }
        for (let row = 0; row < rows; row++) {
            yArray.push(baseY + row);
        }
    },

    addItemsInLine: function(item, endX, endY, tileSize) {
        if (!game.previousDrop) {
            game.previousDrop = { x: endX, y: endY };
            this.calculateTilePositions(this.selectedItem, endX, endY, tileSize, item.x, item.y);
            this.addNewItemToRoomData(item);
            return;
        }

        const startX = game.previousDrop.x;
        const startY = game.previousDrop.y;

        const stepX = endX > startX ? 1 : -1;
        const stepY = endY > startY ? 1 : -1;

        if (startX === endX) {
            for (let y = startY; y !== endY + stepY; y += stepY) {
                this.calculateTilePositions(this.selectedItem, endX, y, tileSize, item.x, item.y);
                this.addNewItemToRoomData({ ...item, y: [y], x: [endX] });
            }
        } else if (startY === endY) {
            for (let x = startX; x !== endX + stepX; x += stepX) {
                this.calculateTilePositions(this.selectedItem, x, endY, tileSize, item.x, item.y);
                this.addNewItemToRoomData({ ...item, y: [endY], x: [x] });
            }
        } else {
            // Handle diagonal placement if needed
        }

        game.previousDrop = { x: endX, y: endY };
    },

    addNewItemToRoomData: function(newItem) {
        if (!game.roomData.items) {
            game.roomData.items = [];
        }

        function isOverlapping(existingItem, newItem) {
            const existingItemData = game.objectData[existingItem.id];
            const newItemData = game.objectData[newItem.id];

            if (!existingItemData || !newItemData) {
                return false;
            }

            const existingTiles = new Set();
            for (let i = 0; i < existingItem.x.length; i++) {
                for (let j = 0; j < existingItem.y.length; j++) {
                    existingTiles.add(`${existingItem.x[i]}_${existingItem.y[j]}`);
                }
            }

            for (let i = 0; i < newItem.x.length; i++) {
                for (let j = 0; j < newItem.y.length; j++) {
                    const tileKey = `${newItem.x[i]}_${newItem.y[j]}`;
                    if (existingTiles.has(tileKey)) {
                        if (game.objectData[existingItem.id][0].z >= game.objectData[newItem.id][0].z) {
                            game.overlappingTiles.push({ x: newItem.x[i], y: newItem.y[j] });
                        }
                    }
                }
            }

            return game.overlappingTiles.length > 0;
        }

        let overlapDetected = false;
        for (let i = 0; i < game.roomData.items.length; i++) {
            const existingItem = game.roomData.items[i];
            if (isOverlapping(existingItem, newItem)) {
                overlapDetected = true;
                break;
            }
        }

        if (overlapDetected && newItem.s !== 1) {
            console.log('Overlap detected, item not added.');
            return;
        }

        game.roomData.items.push(newItem);

        effects.shakeMap(300, 3);

        console.log('New item added to roomData:', newItem);
        audio.playAudio("objectDrop", assets.load('objectDrop'), 'sfx');
        this.saveRoomData();
    },

    pickUpSelectedItems: function() {
        if (game.selectedObjects.length === 0) return;

        // Remove selected items from roomData
        game.roomData.items = game.roomData.items.filter(roomItem => !game.selectedObjects.includes(roomItem));

        // Remove associated light sources and effects
        game.selectedObjects.forEach(item => {
            const xCoordinates = item.x.map(x => parseInt(x, 10) * 16);
            const yCoordinates = item.y.map(y => parseInt(y, 10) * 16);
            xCoordinates.forEach((x, index) => {
                const itemData = game.objectData[item.id];
                if (itemData && itemData.length > 0) {
                    const tileData = itemData[0];
                    if (tileData.l && tileData.l.length > 0) {
                        tileData.l.forEach(light => {
                            if (Array.isArray(light) && light.length === 2) {
                                const lightXIndex = light[0];
                                const lightYIndex = light[1];

                                if (lightXIndex >= 0 && lightXIndex < item.x.length &&
                                    lightYIndex >= 0 && lightYIndex < item.y.length) {

                                    const tileX = item.x[lightXIndex];
                                    const tileY = item.y[lightYIndex];

                                    const lightId = `${item.id}_${tileX}_${tileY}`;
                                    effects.lights = effects.lights.filter(light => light.id !== lightId);
                                }
                            }
                        });
                    }
                }
            });

            // Call removeItemEffects to remove associated effects
            effects.removeItemEffects(item);
        });

        // Highlight the removed item tiles
        game.overlappingTiles = [];
        game.selectedObjects.forEach(item => {
            const xCoordinates = item.x.map(x => parseInt(x, 10) * 16);
            const yCoordinates = item.y.map(y => parseInt(y, 10) * 16);
            xCoordinates.forEach((x, index) => {
                game.overlappingTiles.push({ x: x, y: yCoordinates[index] });
            });
        });

        // Clear selectedObjects
        game.selectedObjects = [];

        // Save the updated room data
        editor.saveRoomData();

        // Re-render the game state to reflect the changes immediately
        game.render();

        console.log('Selected items picked up and associated light sources removed');
    },

    saveRoomData: function() {
        const data = {
            sceneid: game.sceneid,
            roomData: game.roomData
        };
        const dataToSend = JSON.stringify(data);
        console.log('Data being sent to server:', dataToSend);

        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'modals/inventory/ajax/save_map.php',
            data: dataToSend,
            headers: {
                'Content-Type': 'application/json'
            },
            success: function(data) {
                console.log('Room data saved successfully:', data);
            },
            error: function(data) {
                console.error('Error saving room data:', data);
            }
        });
    },

    highlightOverlappingTiles: function(newItem) {
        if (!game.roomData.items) return;

        game.overlappingTiles = [];

        function isOverlapping(existingItem, newItem) {
            const existingItemData = game.objectData[existingItem.id];
            const newItemData = game.objectData[newItem.id];

            if (!existingItemData || !newItemData) {
                return false;
            }

            const existingTiles = new Set();
            for (let i = 0; i < existingItem.x.length; i++) {
                for (let j = 0; j < existingItem.y.length; j++) {
                    existingTiles.add(`${existingItem.x[i]}_${existingItem.y[j]}`);
                }
            }

            for (let i = 0; i < newItem.x.length; i++) {
                for (let j = 0; j < newItem.y.length; j++) {
                    const tileKey = `${newItem.x[i]}_${newItem.y[j]}`;
                    if (existingTiles.has(tileKey)) {
                        if (game.objectData[existingItem.id][0].z >= game.objectData[newItem.id][0].z) {
                            game.overlappingTiles.push({ x: newItem.x[i], y: newItem.y[j] });
                        }
                    }
                }
            }

            return game.overlappingTiles.length > 0;
        }

        for (let i = 0; i < game.roomData.items.length; i++) {
            const existingItem = game.roomData.items[i];
            if (isOverlapping(existingItem, newItem)) {
                break;
            }
        }
    }
};