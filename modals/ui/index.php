<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
<div data-window='ui_window' data-close="false">
  <div class='fixed bottom-0 right-2 z-10 text-sm mb-1 flex space-x-4 tracking-tight'>
    <span class="text-white rounded-md">Renzora v0.0.7</span>
    <span class="text-white rounded-md" id="gameFps"></span>
    <span id="game_time" class="text-white rounded-md">00:00</span>
    <button onclick="ui_window.activateTimeout('quick_item_1', 5000)">Activate Timeout for Item 1</button>
    <button onclick="ui_window.activateTimeout('quick_item_2', 7000)">Activate Timeout for Item 2</button>
    <button onclick="ui_window.activateTimeout('item_primary', 8000)">Activate Timeout for Primary Item</button>
  </div>

  <!-- Top Center: Health, Energy, Quick Items, Avatar -->
  <div class="fixed top-3 left-1/2 transform -translate-x-1/2 z-10 flex space-x-2 tracking-tight bg-[#13181e] rounded-md shadow-inner hover:shadow-lg p-1">
    <div id="item_primary" class="relative flex items-center justify-center w-20 h-18 bg-[#1e2b3d] rounded-md shadow-2xl hover:shadow-2xl transition-shadow duration-300">
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
        <div id="quick_item_1" class="quick_item relative cursor-move w-12 h-12 bg-[#1e2b3d] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
          <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
          <div class="items_icon items_potion scale-[2.1] z-10"></div>
        </div>
        <div id="quick_item_2" class="quick_item relative cursor-move w-12 h-12 bg-[#1e2b3d] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
          <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
          <div class="items_icon items_shield scale-[2.1] z-10"></div>
        </div>
        <div id="quick_item_3" class="quick_item relative cursor-move w-12 h-12 bg-[#1e2b3d] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
          <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
          <div class="items_icon items_sword scale-[2.1] z-10"></div>
        </div>
        <div id="quick_item_4" class="quick_item relative cursor-move w-12 h-12 bg-[#1e2b3d] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
          <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
          <div class="items_icon items_skull scale-[2.1] z-10"></div>
        </div>
        <div id="quick_item_5" class="quick_item relative cursor-move w-12 h-12 bg-[#1e2b3d] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
          <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
          <div class="items_icon items_key scale-[2.1] z-10"></div>
        </div>
      </div>
    </div>
  </div>

  <style>
    .highlight {
      outline: 2px dashed yellow;
      outline-offset: -3px;
    }
  </style>

  <script>
  var ui_window = {
    dragSrcEl: null,

    start: function() {
      this.initializeDragAndDrop();
      this.initializeQuickItems();
      this.initializePrimaryItem();
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
          this.startTimeout(item, 5000); // Example: 5 seconds timeout
        });
      });
    },

    initializePrimaryItem: function() {
      const primaryItem = document.getElementById('item_primary');
      primaryItem.addEventListener('click', () => {
        this.startTimeout(primaryItem, 8000); // Example: 8 seconds timeout for the primary item
      });
    },

    startTimeout: function(item, duration) {
      if (!item.classList.contains('pointer-events-none')) {
        item.classList.add('pointer-events-none', 'opacity-80');
        const indicator = item.querySelector('.timeout-indicator');
        indicator.classList.remove('hidden');
        indicator.style.width = '100%';
        indicator.style.transitionDuration = `${duration}ms`;
        requestAnimationFrame(() => {
          indicator.style.width = '0%';
        });
        setTimeout(() => {
          item.classList.remove('pointer-events-none', 'opacity-80');
          indicator.style.transitionDuration = '0ms'; // Reset transition duration
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
      e.dataTransfer.setData('text/html', this.dragSrcEl.innerHTML);
      this.dragSrcEl.classList.add('dragging');
      this.dragSrcEl.style.cursor = 'move';
    },

    handleDragOver: function(e) {
      if (e.preventDefault) {
        e.preventDefault(); // Necessary. Allows us to drop.
      }
      e.dataTransfer.dropEffect = 'move'; // Show move cursor
      const target = e.target.closest('.quick_item, #item_primary');
      if (target) {
        this.clearHighlights();
        target.classList.add('highlight');
      }
      return false;
    },

    handleDrop: function(e) {
      if (e.stopPropagation) {
        e.stopPropagation(); // Stops some browsers from redirecting.
      }
      const target = e.target.closest('.quick_item, #item_primary');
      if (this.dragSrcEl !== target && target) {
        // Swap the inner HTML of the elements
        const srcInnerHTML = this.dragSrcEl.innerHTML;
        const targetInnerHTML = target.innerHTML;

        // Perform the swap
        this.dragSrcEl.innerHTML = targetInnerHTML;
        target.innerHTML = srcInnerHTML;

        // Adjust the scale
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
    }
  };

  ui_window.start();
  </script>
</div>
<?php
}
?>
