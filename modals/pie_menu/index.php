<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
<div data-window='pie_menu_window' data-close="false">

<div id="pieMenu" class="pie-menu">
    <div class="pie-menu-ring"></div>
    <div class="pie-item" data-action="action1" style="--item-index: 0;">
        <div class="pie-item-content">Edit Mode</div>
    </div>
    <div class="pie-item" data-action="action2" style="--item-index: 1;">
        <div class="pie-item-content">Mishell</div>
    </div>
    <div class="pie-item" data-action="action3" style="--item-index: 2;">
        <div class="pie-item-content">Debug</div>
    </div>
    <div class="pie-item" data-action="action4" style="--item-index: 3;">
        <div class="pie-item-content">Inventory</div>
    </div>
    <div class="pie-item" data-action="action5" style="--item-index: 4;">
        <div class="pie-item-content">Settings</div>
    </div>
    <div class="pie-item" data-action="action6" style="--item-index: 5;">
        <div class="pie-item-content">Toggle Aim</div>
    </div>
</div>

<style>
@keyframes zoomIn {
  from {
    transform: translate(-50%, -50%) scale(0);
    opacity: 0;
  }
  to {
    transform: translate(-50%, -50%) scale(1);
    opacity: 1;
  }
}

@keyframes zoomOut {
  from {
    transform: translate(-50%, -50%) scale(1);
    opacity: 1;
  }
  to {
    transform: translate(-50%, -50%) scale(0);
    opacity: 0;
  }
}

.pie-menu {
  display: none; /* Hidden by default */
  position: fixed;
  top: 50%;
  left: 50%;
  width: 660px;
  height: 660px;
  z-index: 20;
  opacity: 0;
  transform: translate(-50%, -50%) scale(0); /* Start scaled down */
  border-radius: 50%;
  background: rgba(0, 0, 0, 0.9);
  clip-path: circle(50% at 50% 50%);
  cursor: pointer;
  border: 3px solid rgba(0, 0, 0, 0.1);
  overflow: hidden;
  transition: transform 0.5s ease, opacity 0.5s ease;
}

.pie-menu.show {
  animation: zoomIn 0.5s forwards; /* Use forwards to retain the final state */
}

.pie-menu-ring {
  position: absolute;
  width: 40%; /* Adjust for ring size */
  height: 40%; /* Adjust for ring size */
  background: none;
  border-radius: 50%;
  border: 3px solid rgba(0, 0, 0, 0.6); /* Adjust color and opacity */
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
}

.pie-item {
  display: flex;
  align-items: center;
  justify-content: center;
  position: absolute;
  color: #FFF;
  font-size: 28px;
  width: 50%;
  height: 50%;
  bottom: 50%;
  transform-origin: 100% 100%;
  transform: rotate(calc(var(--item-index) * 60deg)) translateX(0%);
}

.pie-item-content {
  display: flex;
  align-items: center;
  justify-content: center;
  transform: rotate(calc(-1 * var(--item-index) * 60deg));
  width: 100%;
  height: 100%;
  text-align: center; /* Ensure text is centered */
}

.pie-item:hover {
  background: rgba(47, 124, 224, 0.7);
}

.pie-item::before {
  content: '';
  display: block;
  width: 24px;
  height: 24px;
  margin-right: 10px;
  background: no-repeat center center;
  background-size: cover;
}

.pie-menu::after {
  content: "";
  position: absolute;
  width: 40%; /* Adjust to match the diameter of the central ring */
  height: 40%; /* Adjust to match the diameter of the central ring */
  background: #131d21; /* Match the page background or the pie-menu background */
  border-radius: 50%;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  z-index: 10; /* Ensure this overlays the slices; adjust as necessary */
}
</style>

<script>
var pie_menu_window = {
    currentHoveredAction: null,
    pieMenuVisible: false,

    start: function() {
        this.initPieMenu();
    },

    unmount: function() {
        this.removePieMenuListeners();
    },

    // Pie Menu Methods
    showPieMenu: function(x, y) {
        const pieMenu = document.getElementById('pieMenu');
        pieMenu.style.left = `${x}px`;
        pieMenu.style.top = `${y}px`;
        pieMenu.style.display = 'flex'; // Make it visible
        // Increase delay to ensure the display change is acknowledged
        setTimeout(() => {
            modal.front('pie_menu_window'); // Pass the correct element
            pieMenu.classList.add('show');
            this.pieMenuVisible = true;
        }, 20);
    },

    // Method to hide the pie menu
    hidePieMenu: function() {
        const pieMenu = document.getElementById('pieMenu');
        pieMenu.classList.remove('show');
        // Wait for animation to complete before hiding
        setTimeout(() => {
            pieMenu.style.display = 'none';
            this.pieMenuVisible = false;
        }, 500);
    },

    // Method to initialize the pie menu
    initPieMenu: function() {
        this.pieMenuKeyDownListener = e => {
            if (e.key === 'Alt' && !e.repeat) {
                e.preventDefault(); // Prevent default Alt key behavior
                this.showPieMenu(e.clientX, e.clientY);
            }
        };

        this.pieMenuKeyUpListener = e => {
            if (e.key === 'Alt') {
                e.preventDefault(); // Prevent default Alt key behavior
                if (this.pieMenuVisible && this.currentHoveredAction) {
                    // Execute the action stored in currentHoveredAction
                    console.log(`Executing action: ${this.currentHoveredAction}`);
                    if (this.currentHoveredAction === 'action1') {
                        modal.load('editMode/index.php', 'edit_mode_window');
                    } else if (this.currentHoveredAction === 'action2') {
                        modal.load('mishell');
                    } else if (this.currentHoveredAction === 'action3') {
                        modal.load('debug');
                    } else if (this.currentHoveredAction === 'action4') {
                        modal.load('inventory');
                    } else if (this.currentHoveredAction === 'action5') {
                        modal.load('settings');
                    } else if (this.currentHoveredAction === 'action6') {
                        const mainSprite = game.sprites['main'];
                        if (mainSprite) {
                            mainSprite.targetAim = !mainSprite.targetAim;
                        } else {
                            console.log('Main sprite not found');
                        }
                    } else {
                        console.log('No action selected');
                    }
                }
                this.hidePieMenu();
            }
        };

        this.pieItemHoverListener = e => {
            this.currentHoveredAction = e.currentTarget.dataset.action;
            console.log(`Hovered over action: ${this.currentHoveredAction}`);
        };

        document.addEventListener('keydown', this.pieMenuKeyDownListener);
        document.addEventListener('keyup', this.pieMenuKeyUpListener);
        
        document.querySelectorAll('.pie-item').forEach(item => {
            item.addEventListener('mouseover', this.pieItemHoverListener);
            item.addEventListener('mouseout', () => {
                this.currentHoveredAction = null;
            });
        });
    },

    removePieMenuListeners: function() {
        document.removeEventListener('keydown', this.pieMenuKeyDownListener);
        document.removeEventListener('keyup', this.pieMenuKeyUpListener);

        document.querySelectorAll('.pie-item').forEach(item => {
            item.removeEventListener('mouseover', this.pieItemHoverListener);
            item.removeEventListener('mouseout', () => {
                this.currentHoveredAction = null;
            });
        });
    }
};

pie_menu_window.start();
</script>

  </div>
<?php
}
?>
