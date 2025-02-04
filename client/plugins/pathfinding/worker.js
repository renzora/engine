let grid = null;
let gridSize = 16;
let objectData = null;

self.onmessage = function(e) {
    const { type } = e.data;
    
    switch(type) {
        case 'initGrid':
            const { worldWidth, worldHeight, roomData, objectData: objData } = e.data;
            gridSize = e.data.gridSize;
            objectData = objData;
            grid = createWalkableGrid(worldWidth, worldHeight, roomData);
            self.postMessage({ type: 'gridCreated', grid });
            break;
            
        case 'findPath':
            const { requestId, startX, startY, endX, endY } = e.data;
            const path = findPath(startX, startY, endX, endY, grid);
            self.postMessage({ requestId, path });
            break;
    }
};

function createWalkableGrid(roomWidth, roomHeight, roomData) {
    const cols = Math.ceil(roomWidth / gridSize);
    const rows = Math.ceil(roomHeight / gridSize);
    const grid = Array(cols).fill().map(() => Array(rows).fill(1));
    
    if (roomData?.items) {
        roomData.items.forEach(item => {
            const itemData = objectData[item.id]?.[0];
            if (!itemData?.w || itemData.w.length === 0) return;

            if (Array.isArray(itemData.w)) {
                const baseX = Math.min(...item.x) * gridSize;
                const baseY = Math.min(...item.y) * gridSize;
                
                const polygonPoints = itemData.w.map(point => ({
                    x: point.x + baseX,
                    y: point.y + baseY
                }));

                for (let x = 0; x < cols; x++) {
                    for (let y = 0; y < rows; y++) {
                        const cellX = x * gridSize;
                        const cellY = y * gridSize;
                        
                        if (pointInPolygon(
                            cellX + gridSize/2,
                            cellY + gridSize/2,
                            polygonPoints
                        )) {
                            grid[x][y] = 0;
                        }
                    }
                }
            }
        });
    }
    
    return grid;
}

function pointInPolygon(x, y, polygon) {
    let inside = false;
    for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
        const xi = polygon[i].x, yi = polygon[i].y;
        const xj = polygon[j].x, yj = polygon[j].y;
        
        const intersect = ((yi > y) !== (yj > y))
            && (x < (xj - xi) * (y - yi) / (yj - yi) + xi);
        if (intersect) inside = !inside;
    }
    return inside;
}

function findPath(startX, startY, endX, endY, grid) {
    const start = {
        x: Math.floor(startX / gridSize),
        y: Math.floor(startY / gridSize)
    };
    const end = {
        x: Math.floor(endX / gridSize),
        y: Math.floor(endY / gridSize)
    };

    if (start.x < 0 || start.y < 0 || end.x < 0 || end.y < 0 ||
        start.x >= grid.length || start.y >= grid[0].length ||
        end.x >= grid.length || end.y >= grid[0].length) {
        return [];
    }

    if (grid[start.x][start.y] === 0 || grid[end.x][end.y] === 0) {
        return [];
    }

    const openSet = new Set();
    const closedSet = new Set();
    const cameFrom = new Map();
    const gScore = new Map();
    const fScore = new Map();
    
    const startKey = `${start.x},${start.y}`;
    openSet.add(startKey);
    gScore.set(startKey, 0);
    fScore.set(startKey, heuristic(start, end));

    while (openSet.size > 0) {
        let current = null;
        let lowestF = Infinity;
        
        for (const key of openSet) {
            const f = fScore.get(key);
            if (f < lowestF) {
                lowestF = f;
                current = key;
            }
        }

        const [currentX, currentY] = current.split(',').map(Number);
        
        if (currentX === end.x && currentY === end.y) {
            return reconstructPath(cameFrom, current);
        }

        openSet.delete(current);
        closedSet.add(current);

        const neighbors = getNeighbors(currentX, currentY, grid);
        
        for (const neighbor of neighbors) {
            const neighborKey = `${neighbor.x},${neighbor.y}`;
            
            if (closedSet.has(neighborKey)) continue;
            
            const tentativeG = gScore.get(current) + 1;
            
            if (!openSet.has(neighborKey)) {
                openSet.add(neighborKey);
            } else if (tentativeG >= gScore.get(neighborKey)) {
                continue;
            }

            cameFrom.set(neighborKey, current);
            gScore.set(neighborKey, tentativeG);
            fScore.set(neighborKey, tentativeG + heuristic(neighbor, end));
        }
    }
    return [];
}

function getNeighbors(x, y, grid) {
    const neighbors = [];
    const directions = [
        {x: -1, y: 0}, {x: 1, y: 0}, {x: 0, y: -1}, {x: 0, y: 1},
        {x: -1, y: -1}, {x: -1, y: 1}, {x: 1, y: -1}, {x: 1, y: 1}
    ];

    for (const dir of directions) {
        const newX = x + dir.x;
        const newY = y + dir.y;

        if (newX >= 0 && newX < grid.length &&
            newY >= 0 && newY < grid[0].length &&
            grid[newX][newY] === 1) {
            neighbors.push({x: newX, y: newY});
        }
    }

    return neighbors;
}

function heuristic(a, b) {
    return Math.abs(a.x - b.x) + Math.abs(a.y - b.y);
}

function reconstructPath(cameFrom, current) {
    const path = [];
    while (cameFrom.has(current)) {
        const [x, y] = current.split(',').map(Number);
        path.unshift({
            x: x * gridSize + gridSize/2,
            y: y * gridSize + gridSize/2
        });
        current = cameFrom.get(current);
    }
    return path;
}