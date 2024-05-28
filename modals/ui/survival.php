<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
<div data-window='survival_window'>

    <script>
var survival_window = {
    requiredClicks: 10, // Number of clicks required to chop down a tree
    currentClicks: 0,  // Counter for the current number of clicks
    currentTreeIndex: null, // Current tree being chopped
    choppingBar: null, // Reference to the chopping bar element

    start: function() {
        this.addEventListeners();
        console.log('Survival window started.');
        ui.notif("Welcome to survival mode!", "bottom-center")
    },

    unmount: function() {
        this.removeEventListeners();
        console.log('Survival window unmounted.');
    },

    addEventListeners: function() {
        document.addEventListener('keydown', this.keyDownHandler.bind(this));
        document.addEventListener('click', this.clickHandler.bind(this));
    },

    removeEventListeners: function() {
        document.removeEventListener('keydown', this.keyDownHandler.bind(this));
        document.removeEventListener('click', this.clickHandler.bind(this));
    },

    keyDownHandler: function(e) {
        if (e.key === 'f' || e.key === 'F') {
            this.attemptChopTree();
        }
    },

    clickHandler: function(e) {
        this.attemptChopTree();
    },

    attemptChopTree: function() {
        const facingTreeIndex = this.isFacingTree(game.sprites['main'], game.roomData);
        if (facingTreeIndex !== -1) {
            console.log('Sprite is facing a tree at index:', facingTreeIndex);

            if (this.currentTreeIndex === null || this.currentTreeIndex !== facingTreeIndex) {
                this.currentTreeIndex = facingTreeIndex;
                this.currentClicks = 0; // Reset click counter for new tree
                this.showChoppingBar(facingTreeIndex);
            }

            this.currentClicks++;
            console.log(`Chop attempt ${this.currentClicks}/${this.requiredClicks}`);

            const tree = game.roomData.items[facingTreeIndex];
            game.utils.shakeItem('tree', tree.x[0], tree.y[0], 1, 100); // Shake the tree with intensity 1

            const progress = 100 - (this.currentClicks / this.requiredClicks) * 100;
            this.updateChoppingBar(progress);
            this.showFloatingXP(this.getTreePosition(facingTreeIndex));

            if (this.currentClicks >= this.requiredClicks) {
                this.scatterAndRemoveTree(facingTreeIndex);
                this.currentTreeIndex = null; // Reset current tree
            }
        } else {
            console.log('No tree in the direction the sprite is facing.');
            this.resetChoppingState();
        }
    },

    isFacingTree: function(sprite, roomData) {
        const directionOffsets = {
            'N': { x: 0, y: -1 },
            'S': { x: 0, y: 1 },
            'E': { x: 1, y: 0 },
            'W': { x: -1, y: 0 },
        };
        const offset = directionOffsets[sprite.direction];
        const targetX = Math.floor((sprite.x + offset.x * 16) / 16);
        const targetY = Math.floor((sprite.y + offset.y * 16) / 16);

        return roomData.items.findIndex(item => 
            item.id === 'tree' && item.x.includes(targetX) && item.y.includes(targetY)
        );
    },

    showChoppingBar: function(treeIndex) {
        console.log('Chopping tree at index:', treeIndex);
        if (this.choppingBar) {
            document.body.removeChild(this.choppingBar);
        }

        const treePosition = this.getTreePosition(treeIndex);
        const choppingBar = document.createElement('div');
        choppingBar.id = 'chopping-bar';
        choppingBar.style.position = 'absolute';
        choppingBar.style.left = `${treePosition.x}px`;
        choppingBar.style.top = `${treePosition.y - 20}px`; // Adjust position above the tree
        choppingBar.style.width = `${40 * game.zoomLevel}px`; // Adjust width based on zoom level
        choppingBar.style.height = `${6 * game.zoomLevel}px`; // Adjust height based on zoom level
        choppingBar.style.backgroundColor = 'rgba(0, 0, 0, 0.8)'; // More opaque black
        choppingBar.style.border = '1px solid #333';
        choppingBar.style.borderRadius = '4px';
        choppingBar.style.padding = '2px'; // Add padding
        choppingBar.style.boxShadow = '0px 0px 5px rgba(0, 0, 0, 0.5)';
        choppingBar.style.transform = 'translateX(-50%)'; // Center the bar

        const progressBar = document.createElement('div');
        progressBar.style.width = '100%';
        progressBar.style.height = '100%';
        progressBar.style.background = 'linear-gradient(to right, #0a0, #0a0)';
        progressBar.style.border = '1px solid #000'; // Dark border for progress bar
        progressBar.style.borderRadius = '4px';
        progressBar.style.transition = 'width 0.3s ease'; // Add transition for smooth sliding

        choppingBar.appendChild(progressBar);
        document.body.appendChild(choppingBar);

        this.choppingBar = choppingBar;
    },

    updateChoppingBar: function(progress) {
        const progressBar = this.choppingBar.firstChild;
        progressBar.style.width = `${progress}%`;

        if (progress > 50) {
            progressBar.style.background = 'linear-gradient(to right, #0a0, #0a0)'; // Green gradient
        } else if (progress > 25) {
            progressBar.style.background = 'linear-gradient(to right, #ffa500, #ffa500)'; // Orange gradient
        } else {
            progressBar.style.background = 'linear-gradient(to right, #f00, #f00)'; // Red gradient
        }

        if (progress <= 0) {
            document.body.removeChild(this.choppingBar);
            this.choppingBar = null;
        }
    },

    showFloatingXP: function(treePosition) {
        const xpText = document.createElement('div');
        xpText.innerText = '+10 XP';
        xpText.style.position = 'absolute';
        xpText.style.left = `${treePosition.x}px`;
        xpText.style.top = `${treePosition.y - 30}px`; // Adjust position above the tree
        xpText.style.color = '#fff';
        xpText.style.fontSize = '12px';
        xpText.style.fontWeight = 'bold';
        xpText.style.transform = 'translateX(-50%)';
        xpText.style.transition = 'top 1s ease, opacity 1s ease';
        xpText.style.opacity = '1';

        document.body.appendChild(xpText);

        setTimeout(() => {
            xpText.style.top = `${treePosition.y - 50}px`; // Move up
            xpText.style.opacity = '0'; // Fade out
            setTimeout(() => document.body.removeChild(xpText), 1000);
        }, 0);
    },

    getTreePosition: function(treeIndex) {
        const tree = game.roomData.items[treeIndex];
        const treeX = Math.min(...tree.x) * 16;
        const treeY = Math.min(...tree.y) * 16;
        const canvasX = (treeX - game.cameraX) * game.zoomLevel;
        const canvasY = (treeY - game.cameraY) * game.zoomLevel;
        return { x: canvasX, y: canvasY };
    },

    scatterAndRemoveTree: function(treeIndex) {
        const tree = game.roomData.items[treeIndex];
        const treeX = tree.x[0];
        const treeY = tree.y[0];
        game.utils.scatterItem('tree', treeX, treeY, 3); // Scatter the tree tiles
        setTimeout(() => {
            game.roomData.items.splice(treeIndex, 1); // Remove the tree item from roomData
            console.log(`Tree removed at index ${treeIndex}`);
            this.resetChoppingState();
        }, 500); // Allow time for the scatter animation before removing
    },

    resetChoppingState: function() {
        this.currentClicks = 0;
        this.currentTreeIndex = null;
        if (this.choppingBar) {
            document.body.removeChild(this.choppingBar);
            this.choppingBar = null;
        }
    }
};

survival_window.start();

    </script>

  </div>
<?php
}
?>