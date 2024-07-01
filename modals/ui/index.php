<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
<div data-window='ui_window' data-close="false">

<div id="ui_window_objectives" class="w-72 fixed top-1/4 right-2 z-10 flex rounded flex-col tracking-tight bg-[#137966] bg-opacity-70 shadow-md fade-edges">
    <div id="objectives_container" class="flex flex-col space-y-1 p-2">
        <!-- Objectives will be dynamically inserted here by the displayObjectives method -->
    </div>
</div>

<div class='fixed bottom-3 left-2 text-sm mb-1 '>
<button class="green_button text-white font-bold py-1 px-2 rounded shadow-md" onclick="modal.load('quick_menu');">Console</button>
</div>

  <div class='fixed bottom-0 right-2 z-10 text-sm mb-1 flex space-x-4 tracking-tight'>
    <span class="text-white rounded-md">Renzora v0.0.7</span>
    <span class="text-white rounded-md" id="gameFps"></span>
    <span id="game_time" class="text-white rounded-md">00:00</span>
    <button onclick="modal.load('minimap');">Minimap</button>
  </div>

  <!-- Top Center: Health, Energy, Quick Items, Avatar -->
  <div id="ui_window_inventory" class="fixed bottom-4 left-1/2 transform -translate-x-1/2 z-10 flex space-x-2 tracking-tight bg-[#0a0d14] rounded-md shadow-inner hover:shadow-lg p-1 border border-black">
    <div id="item_primary" class="relative flex items-center justify-center w-20 h-18 bg-[#18202f] rounded-md shadow-2xl hover:shadow-2xl transition-shadow duration-300">
      <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
      <div class="items_icon items_sword scale-[4] z-10"></div>
    </div>
    <div class="flex flex-col space-y-2 w-98">
      <div class="flex items-center space-x-2 w-full">
        <div class="relative w-1/2 bg-gray-900 rounded-md h-6 overflow-hidden shadow-inner bg-opacity-80 shadow-sm p-[1px] flex items-center">
          <div class="mx-1">
            <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
            <div class="items_icon items_health scale-[1.2]"></div>
          </div>
          <div id="health" class="rounded bg-gradient-to-r from-lime-500 to-green-600 h-full transition-width duration-500 flex-grow"></div>
          <div class="absolute inset-0 flex items-center pl-8 text-white text-sm">0%</div>
        </div>
        <div class="relative w-1/2 bg-gray-900 rounded-md h-6 overflow-hidden shadow-inner bg-opacity-80 shadow-sm p-[1px] flex items-center">
          <div class="mx-1">
            <div class="items_icon items_energy scale-[1.2]"></div>
          </div>
          <div id="energy" class="rounded bg-gradient-to-r from-cyan-400 to-blue-600 h-full transition-width duration-500 flex-grow"></div>
          <div class="absolute inset-0 flex items-center pl-8 text-white text-sm">0%</div>
        </div>
      </div>
      <!-- Quick Items -->
      <div class="flex space-x-2" id="quick_items_container">
        <div id="quick_item_1" class="quick_item relative cursor-move w-12 h-12 bg-[#18202f] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
          <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
          <div class="items_icon items_potion scale-[2.1] z-10"></div>
        </div>
        <div id="quick_item_2" class="quick_item relative cursor-move w-12 h-12 bg-[#18202f] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
          <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
          <div class="items_icon items_shield scale-[2.1] z-10"></div>
        </div>
        <div id="quick_item_3" class="quick_item relative cursor-move w-12 h-12 bg-[#18202f] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
          <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
          <div class="items_icon items_banana scale-[2.1] z-10"></div>
        </div>
        <div id="quick_item_4" class="quick_item relative cursor-move w-12 h-12 bg-[#18202f] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
          <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
          <div class="items_icon items_skull scale-[2.1] z-10"></div>
        </div>
        <div id="quick_item_5" class="quick_item relative cursor-move w-12 h-12 bg-[#18202f] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
          <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
          <div class="items_icon items_key scale-[2.1] z-10"></div>
        </div>
        <div id="quick_item_6" class="quick_item relative cursor-move w-12 h-12 bg-[#18202f] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
          <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
          <div class="items_icon items_gold scale-[2.1] z-10"></div>
        </div>
        <div id="quick_item_7" class="quick_item relative cursor-move w-12 h-12 bg-[#18202f] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
          <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
          <div class="items_icon items_apple scale-[2.1] z-10"></div>
        </div>
      </div>
    </div>
  </div>


  <style>
  .highlight {
    outline: 2px dashed yellow;
    outline-offset: -3px;
  }

  /* Custom styles for the checkbox */
  .custom-checkbox {
    position: relative;
    width: 18px;
    height: 18px;
    background-color: white;
    border-radius: 2px;
    margin: 3px 0 3px 3px;
  }

  .custom-checkbox input {
    opacity: 0;
    width: 100%;
    height: 100%;
    position: absolute;
    left: 0;
    top: 0;
    cursor: pointer;
  }

  .custom-checkbox input:checked + .checkmark {
    background-color: #35d357;
    border: 1px solid black;
  }

  .custom-checkbox input:checked + .checkmark::after {
    content: '';
    position: absolute;
    left: 6px;
    top: 3px;
    width: 4px;
    height: 9px;
    border: solid green;
    border-width: 0 2px 2px 0;
    transform: rotate(45deg);
  }

  .checkmark {
    position: absolute;
    top: 0;
    left: 0;
    height: 100%;
    width: 100%;
    border-radius: 2px;
    border: 1px solid black;
  }
  </style>

<script>
var ui_window = {
  dragSrcEl: null,
  dragClone: null,

  start: function() {
    this.initializeDragAndDrop();
    this.initializeQuickItems();
    this.initializePrimaryItem();
    this.displayObjectives();

    if (game.itemsData && game.itemsImg) {
      this.displayInventoryItems();
    } else {
      console.error("itemsData or itemsImg is not loaded.");
    }

    this.checkAndUpdateUIPositions();
    ui_window.checkObjective("Find the hidden sword");

    document.addEventListener('dragover', ui_window.documentDragOverHandler);
document.addEventListener('drop', ui_window.documentDropHandler);
  },

  unmount: function() {
    document.removeEventListener('dragover', this.documentDragOverHandler);
    document.removeEventListener('drop', this.documentDropHandler);
    this.dragClone = null;
  },

  documentDragOverHandler: function(e) {
    e.preventDefault();
    if (ui_window.dragClone) {
      ui_window.dragClone.style.top = `${e.clientY}px`;
      ui_window.dragClone.style.left = `${e.clientX}px`;
    }
  },

  documentDropHandler: function(e) {
    if (e.stopPropagation) {
      e.stopPropagation();
    }
    if (ui_window.dragClone) {
      document.body.removeChild(ui_window.dragClone);
      ui_window.dragClone = null;
    }
  },

  displayInventoryItems: function() {
    if (!game.itemsData || !game.itemsData.items) {
      console.error("itemsData or items array is not defined.");
      return;
    }

    const inventoryItems = [
      "sword",
      "shield",
      "key",
      "skull",
      "wood",
      "black_emerald",
      "apple",
      "banana"
    ];

    inventoryItems.forEach((itemName, index) => {
      const itemData = game.itemsData.items.find(item => item.name === itemName);
      if (itemData) {
        let itemElement;
        if (index === 0) {
          itemElement = document.getElementById('item_primary'); // Primary item
        } else {
          itemElement = document.getElementById(`quick_item_${index}`); // Quick items
        }

        if (itemElement) {
          this.setItemIcon(itemElement, itemData);
          itemElement.dataset.cd = itemData.cd; // Set cooldown data attribute
        }
      }
    });
  },

  setItemIcon: function(element, itemData) {
    const iconDiv = element.querySelector('.items_icon');
    if (iconDiv) {
      const iconSize = 16; // Adjust based on your icon size
      const canvas = document.createElement('canvas');
      const ctx = canvas.getContext('2d');

      canvas.width = iconSize;
      canvas.height = iconSize;

      ctx.drawImage(
        game.itemsImg, 
        itemData.x, itemData.y, 
        iconSize, iconSize, 
        0, 0, 
        iconSize, iconSize
      );

      const dataURL = canvas.toDataURL();
      iconDiv.style.backgroundImage = `url(${dataURL})`;
      iconDiv.style.width = `${iconSize}px`;
      iconDiv.style.height = `${iconSize}px`;
      iconDiv.style.backgroundSize = 'cover';
    }
  },

  checkAndUpdateUIPositions: function() {
    const sprite = game.sprites[game.playerid];
    if (!sprite) return;

    const thresholdY = game.worldHeight - 50; // Adjust this value as needed
    const thresholdX = game.worldWidth - 80;  // Adjust this value as needed

    const inventoryElement = document.getElementById('ui_window_inventory');
    if (sprite.y > thresholdY) {
      inventoryElement.classList.add('top-4');
      inventoryElement.classList.remove('bottom-4');
    } else {
      inventoryElement.classList.add('bottom-4');
      inventoryElement.classList.remove('top-4');
    }

    const objectivesElement = document.getElementById('ui_window_objectives');
    if (sprite.x > thresholdX) {
      objectivesElement.classList.add('left-2');
      objectivesElement.classList.remove('right-2');
    } else {
      objectivesElement.classList.add('right-2');
      objectivesElement.classList.remove('left-2');
    }
  },

  checkObjective: function(objectiveName) {
    const objective = game.objectives.find(obj => obj.name === objectiveName);
    if (objective) {
      objective.status = true;
      this.displayObjectives();
    }
  },

  displayObjectives: function() {
    const objectivesContainer = document.getElementById('objectives_container');
    if (objectivesContainer) {
      objectivesContainer.innerHTML = '';
      game.objectives.forEach(obj => {
        const objectiveItem = document.createElement('div');
        objectiveItem.classList.add('flex', 'items-start', 'space-x-2');

        const customCheckbox = document.createElement('div');
        customCheckbox.classList.add('custom-checkbox', 'relative', 'flex-shrink-0', 'mt-2');

        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.checked = obj.status;
        checkbox.disabled = true;

        const checkmark = document.createElement('span');
        checkmark.classList.add('checkmark');

        customCheckbox.appendChild(checkbox);
        customCheckbox.appendChild(checkmark);

        const label = document.createElement('label');
        label.textContent = obj.name;
        label.classList.add('text-white', 'flex-1', 'break-words');

        objectiveItem.appendChild(customCheckbox);
        objectiveItem.appendChild(label);

        objectivesContainer.appendChild(objectiveItem);
      });
    }
  },

  initializeDragAndDrop: function() {
    const draggableItems = document.querySelectorAll('.quick_item, #item_primary');

    draggableItems.forEach(item => {
      item.setAttribute('draggable', true);
      item.style.cursor = 'grab';
      item.addEventListener('mouseover', this.handleMouseOver.bind(this));
      item.addEventListener('mouseout', this.handleMouseOut.bind(this));
      item.addEventListener('dragstart', this.handleDragStart.bind(this));
      item.addEventListener('dragover', this.handleDragOver.bind(this));
      item.addEventListener('drop', this.handleDrop.bind(this));
      item.addEventListener('dragend', this.handleDragEnd.bind(this));
    });
  },

  initializeQuickItems: function() {
    const quickItems = document.querySelectorAll('.quick_item');
    quickItems.forEach(item => {
      item.addEventListener('click', () => {
        const cooldown = parseInt(item.dataset.cd, 10) * 1000;
        if (cooldown > 0) {
          this.startTimeout(item, cooldown);
        }
      });
    });
  },

  initializePrimaryItem: function() {
    const primaryItem = document.getElementById('item_primary');
    primaryItem.addEventListener('click', () => {
      const cooldown = parseInt(primaryItem.dataset.cd, 10) * 1000;
      if (cooldown > 0) {
        this.startTimeout(primaryItem, cooldown);
      }
    });
  },

  startTimeout: function(item, duration) {
    if (!item.classList.contains('pointer-events-none')) {
      item.classList.add('pointer-events-none', 'opacity-80');
      const indicator = item.querySelector('.timeout-indicator');
      indicator.classList.remove('hidden');
      indicator.style.width = '100%';
      indicator.style.transitionDuration = `${duration}ms`;

      setTimeout(() => {
        indicator.style.width = '0%';
      }, 10);

      setTimeout(() => {
        item.classList.remove('pointer-events-none', 'opacity-80');
        indicator.style.transitionDuration = '0ms';
        indicator.style.width = '100%';
        indicator.classList.add('hidden');
      }, duration);
    }
  },

  activateTimeout: function(itemId, duration) {
    const item = document.getElementById(itemId);
    if (item) {
      this.startTimeout(item, duration);
    } else {
      console.error(`Item with id ${itemId} not found`);
    }
  },

  handleMouseOver: function(e) {
    e.target.style.cursor = 'grab';
  },

  handleMouseOut: function(e) {
    e.target.style.cursor = 'default';
  },

  handleDragStart: function(e) {
    this.dragSrcEl = e.target.closest('.quick_item, #item_primary');
  e.dataTransfer.effectAllowed = 'move';

  const iconDiv = this.dragSrcEl.querySelector('.items_icon');
  if (iconDiv) {
    const clonedIcon = iconDiv.cloneNode(true);
    clonedIcon.style.position = 'absolute';
    clonedIcon.style.top = `${e.clientY}px`;
    clonedIcon.style.left = `${e.clientX}px`;
    clonedIcon.style.pointerEvents = 'none';
    clonedIcon.style.zIndex = '1000';
    clonedIcon.classList.add('scale-[4]'); // Increase the scale to 4

    this.dragClone = clonedIcon;
    document.body.appendChild(clonedIcon);
  }

  var img = new Image();
  img.src = '';
  e.dataTransfer.setDragImage(img, 0, 0);
  },

  handleDragOver: function(e) {
    if (e.preventDefault) {
      e.preventDefault();
    }
    e.dataTransfer.dropEffect = 'move';

    if (this.dragClone) {
      this.dragClone.style.top = `${e.clientY}px`;
      this.dragClone.style.left = `${e.clientX}px`;
    }

    const target = e.target.closest('.quick_item, #item_primary');
    if (target) {
      this.clearHighlights();
      target.classList.add('highlight');
    } else {
      this.clearHighlights();
    }
    return false;
  },

  handleDrop: function(e) {
    if (e.stopPropagation) {
      e.stopPropagation();
    }
    const target = e.target.closest('.quick_item, #item_primary');
    if (this.dragSrcEl !== target && target) {
      const srcInnerHTML = this.dragSrcEl.innerHTML;
      const targetInnerHTML = target.innerHTML;

      this.dragSrcEl.innerHTML = targetInnerHTML;
      target.innerHTML = srcInnerHTML;

      this.updateScale(this.dragSrcEl);
      this.updateScale(target);
    }
    this.clearHighlights();
    return false;
  },

  clearHighlights: function() {
    const draggableItems = document.querySelectorAll('.quick_item, #item_primary');
    draggableItems.forEach(item => {
      item.classList.remove('highlight');
    });
  },

  updateScale: function(element) {
    const icon = element.querySelector('.items_icon');
    if (element.id === 'item_primary') {
      icon.classList.remove('scale-[2.1]');
      icon.classList.add('scale-[4]');
    } else {
      icon.classList.remove('scale-[4]');
      icon.classList.add('scale-[2.1]');
    }
  },

  handleDragEnd: function(e) {
    const draggableItems = document.querySelectorAll('.quick_item, #item_primary');
    draggableItems.forEach(item => {
      item.classList.remove('dragging');
      item.style.cursor = 'grab';
      item.classList.remove('highlight');
    });

    if (this.dragClone) {
      document.body.removeChild(this.dragClone);
      this.dragClone = null;
    }
  }
};

ui_window.start();
</script>

</div>
<?php
}
?>
