var editor = {
    editMode: false,
    currentMode: null,
    history: [],
    redoStack: [],
    selectionStart: null,
    selectionEnd: null,
    isSelecting: false,
    selectedTiles: [],
    tempSelectedTiles: [],

    undo: function() {
        if (this.history.length > 0) {
            const lastState = this.history.pop();
            this.redoStack.push([...this.selectedTiles]);
            this.selectedTiles = lastState;
            console.log("Undo performed. Selected tiles:", this.selectedTiles);
        }
    },

    redo: function() {
        if (this.redoStack.length > 0) {
            const nextState = this.redoStack.pop();
            this.history.push([...this.selectedTiles]);
            this.selectedTiles = nextState;
            console.log("Redo performed. Selected tiles:", this.selectedTiles);
        }
    },

    updateSelectedTiles: function(isLineSelect = false) {
        let newSelection = [];
        
        if (isLineSelect) {
            const x1 = this.selectionStart.x;
            const y1 = this.selectionStart.y;
            const x2 = this.selectionEnd.x;
            const y2 = this.selectionEnd.y;
            
            const dx = Math.abs(x2 - x1);
            const dy = Math.abs(y2 - y1);
            const sx = (x1 < x2) ? 16 : -16;
            const sy = (y1 < y2) ? 16 : -16;
            let err = dx - dy;

            let x = x1;
            let y = y1;

            while (true) {
                newSelection.push({ x: x, y: y });

                if (x === x2 && y === y2) break;

                const e2 = 2 * err;

                if (e2 > -dy) {
                    err -= dy;
                    x += sx;
                }

                if (e2 < dx) {
                    err += dx;
                    y += sy;
                }
            }
        } else {
            const startX = Math.min(this.selectionStart.x, this.selectionEnd.x);
            const startY = Math.min(this.selectionStart.y, this.selectionEnd.y);
            const endX = Math.max(this.selectionStart.x, this.selectionEnd.x);
            const endY = Math.max(this.selectionStart.y, this.selectionEnd.y);

            for (let x = startX; x <= endX; x += 16) {
                for (let y = startY; y <= endY; y += 16) {
                    newSelection.push({ x: x, y: y });
                }
            }
        }

        if (input.isAltPressed) {
            this.selectedTiles = [...this.tempSelectedTiles, ...newSelection];
        } else {
            this.selectedTiles = newSelection;
        }

        console.log("Selected tiles:", this.selectedTiles);
    }
}