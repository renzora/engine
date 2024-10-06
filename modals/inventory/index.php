<div data-window='inventory_items_window' class='window window_bg' style='width: 500px; background: #232e43;'>
  <div data-part='handle' class='window_title' style='background-image: radial-gradient(#3e5279 1px, transparent 0) !important;'>
    <div class='float-right'>
      <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
    </div>
    <div data-part='title' class='title_bg window_border' style='background: #232e43; color: #ede8d6;'>Inventory</div>
  </div>
  <div class='clearfix'></div>
  <div class='relative'>
    <div class='container text-light window_body p-2'>
      <!-- Tab Buttons -->
      <div id="inventory_items_window_tabs">
        <div id="tabs" class="flex border-b border-gray-300">
          <button class="tab text-gray-800" data-tab="tab_items">Items</button>
          <button class="tab text-gray-800" data-tab="tab_weapons">Weapons</button>
          <button class="tab text-gray-800" data-tab="tab_armour">Armour</button>
          <button class="tab text-gray-800" data-tab="tab_cards">Cards</button>
        </div>

        <!-- Tab Content -->
        <div class="tab-content mt-4">
          <!-- Items Tab -->
          <div id="tab_items" class="tab-panel hidden">
            <div id="main_inventory_items" class="grid grid-cols-4 gap-2">
              <!-- Items will be populated here -->
            </div>
          </div>

          <!-- Weapons Tab -->
          <div id="tab_weapons" class="tab-panel hidden">
            <div id="main_inventory_weapons" class="grid grid-cols-4 gap-2">
              <!-- Weapons will be populated here -->
            </div>
          </div>

          <!-- Armour Tab -->
          <div id="tab_armour" class="tab-panel hidden">
            <div id="main_inventory_armour" class="grid grid-cols-4 gap-2">
              <!-- Armour will be populated here -->
            </div>
          </div>

          <!-- Cards Tab -->
          <div id="tab_cards" class="tab-panel hidden">
            <div id="main_inventory_cards" class="grid grid-cols-4 gap-2">
              <!-- Cards will be populated here -->
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>

  <script>
    var inventory_items_window = {
      items: [
        { name: "potion", type: "item" },
        { name: "banana", type: "item" },
        { name: "wood", type: "item" },
        { name: "apple", type: "item" },
      ],
      weapons: [
        { name: "sword", type: "weapon" },
        { name: "bow", type: "weapon" },
        { name: "dagger", type: "weapon" },
      ],
      armour: [
        { name: "shield", type: "armour" },
        { name: "helmet", type: "armour" },
        { name: "chestplate", type: "armour" },
      ],
      cards: [
        { name: "black_emerald", type: "card" },
        { name: "gem", type: "card" },
        { name: "book", type: "card" },
      ],

      start: function() {
        this.renderTabItems();
        this.initializeDragAndDrop();
        this.initializeTabs();

        // Initialize the tab system with the first tab selected
        ui.initTabs('inventory_items_window_tabs', 'tab_items');
      },

      renderTabItems: function() {
        this.populateItems("main_inventory_items", this.items);
        this.populateItems("main_inventory_weapons", this.weapons);
        this.populateItems("main_inventory_armour", this.armour);
        this.populateItems("main_inventory_cards", this.cards);
      },

      populateItems: function(containerId, items) {
        const container = document.getElementById(containerId);
        container.innerHTML = '';

        items.forEach(item => {
          const itemElement = document.createElement('div');
          itemElement.className = 'main_inventory_item relative cursor-move w-16 h-16 bg-[#18202f] rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center';
          itemElement.dataset.item = item.name;
          itemElement.innerHTML = `
            <div class="timeout-indicator absolute inset-0 bg-red-500 transition-all ease-linear z-0 hidden rounded-md"></div>
            <div class="items_icon items_${item.name} scale-[2.1] z-10"></div>
          `;
          container.appendChild(itemElement);
        });
      },

      initializeTabs: function() {
        const tabs = document.querySelectorAll('#tabs .tab');
        const tabPanels = document.querySelectorAll('.tab-panel');

        tabs.forEach(tab => {
          tab.addEventListener('click', function() {
            const selectedTab = this.getAttribute('data-tab');
            
            tabs.forEach(t => t.classList.remove('active'));
            tabPanels.forEach(panel => panel.classList.add('hidden'));

            document.getElementById(selectedTab).classList.remove('hidden');
            this.classList.add('active');
          });
        });
      },

      initializeDragAndDrop: function() {
        const draggableItems = document.querySelectorAll('.main_inventory_item, .ui_quick_item, .ui_item_primary');

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

      handleMouseOver: function(e) {
        e.target.style.cursor = 'grab';
      },

      handleMouseOut: function(e) {
        e.target.style.cursor = 'default';
      },

      handleDragStart: function(e) {
        this.dragSrcEl = e.target.closest('.main_inventory_item, .ui_quick_item, .ui_item_primary');
        e.dataTransfer.effectAllowed = 'move';

        const iconDiv = this.dragSrcEl.querySelector('.items_icon');
        if (iconDiv) {
          const clonedIcon = iconDiv.cloneNode(true);
          const dragWrapper = document.createElement('div');
          dragWrapper.style.position = 'absolute';
          dragWrapper.style.top = `${e.clientY}px`;
          dragWrapper.style.left = `${e.clientX}px`;
          dragWrapper.style.pointerEvents = 'none';
          dragWrapper.style.zIndex = '1000';
          clonedIcon.style.transform = 'scale(4)';

          dragWrapper.appendChild(clonedIcon);
          this.dragClone = dragWrapper;
          document.body.appendChild(dragWrapper);
        }

        // Show grabbing cursor
        e.target.style.cursor = 'grabbing';

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

        const target = e.target.closest('.main_inventory_item, .ui_quick_item, .ui_item_primary');
        if (target) {
          this.clearHighlights();
          target.classList.add('border-2', 'border-dashed', 'border-yellow-500');
          if (!this.hasPlayedDragOverSound || this.lastHoveredSlot !== target) {
            audio.playAudio("menuDrop", assets.load('menuDrop'), 'sfx', false);
            this.hasPlayedDragOverSound = true;
            this.lastHoveredSlot = target;
          }
        } else {
          this.clearHighlights();
          this.hasPlayedDragOverSound = false;
          this.lastHoveredSlot = null;
        }
        return false;
      },

      handleDrop: function(e) {
        if (e.stopPropagation) {
          e.stopPropagation();
        }
        const target = e.target.closest('.main_inventory_item, .ui_quick_item, .ui_item_primary');
        if (this.dragSrcEl !== target && target) {
          // Swap innerHTML to visually switch the items
          const tempInnerHTML = this.dragSrcEl.innerHTML;
          this.dragSrcEl.innerHTML = target.innerHTML;
          target.innerHTML = tempInnerHTML;

          // Swap dataset item to reflect the changes
          const tempDataItem = this.dragSrcEl.dataset.item;
          this.dragSrcEl.dataset.item = target.dataset.item;
          target.dataset.item = tempDataItem;

          // Update the respective inventory arrays
          this.updateInventoryArray(tempDataItem, this.dragSrcEl.dataset.item);
          this.updateInventoryArray(target.dataset.item, tempDataItem);

          // Update currentItemIndex if needed
          ui_inventory_window.currentItemIndex = ui_inventory_window.getCurrentPrimaryItemIndex();

          // Update the scale of the icons
          ui_inventory_window.updateScale(this.dragSrcEl);
          ui_inventory_window.updateScale(target);

          audio.playAudio("sceneDrop", assets.load('sceneDrop'), 'sfx', false);
        } else {
          audio.playAudio("slotDrop", assets.load('slotDrop'), 'sfx', false);
        }
        this.clearHighlights();
        return false;
      },

      updateInventoryArray: function(oldItem, newItem) {
        const updateArray = (array, oldItem, newItem) => {
          const index = array.findIndex(i => i.name === oldItem);
          if (index !== -1) {
            array[index].name = newItem;
          }
        };

        // Update in all categories
        updateArray(this.items, oldItem, newItem);
        updateArray(this.weapons, oldItem, newItem);
        updateArray(this.armour, oldItem, newItem);
        updateArray(this.cards, oldItem, newItem);

        // Also update the quick inventory
        const quickIndex = ui_inventory_window.inventoryItems.indexOf(oldItem);
        if (quickIndex !== -1) {
          ui_inventory_window.inventoryItems[quickIndex] = newItem;
        }
      },

      handleDragEnd: function(e) {
        const draggableItems = document.querySelectorAll('.main_inventory_item, .ui_quick_item, .ui_item_primary');
        draggableItems.forEach(item => {
          item.classList.remove('dragging');
          item.style.cursor = 'grab'; // Reset the cursor to grab
          item.classList.remove('highlight');
        });

        if (this.dragClone) {
          document.body.removeChild(this.dragClone);
          this.dragClone = null;
        }
      },

      clearHighlights: function() {
        const draggableItems = document.querySelectorAll('.main_inventory_item, .ui_quick_item, .ui_item_primary');
        draggableItems.forEach(item => {
          item.classList.remove('border-2', 'border-dashed', 'border-yellow-500');
          item.style.backgroundColor = ''; // Clear background color
        });
        ui_inventory_window.isItemSelected = false; // Reset the flag when items are deselected
      },

      unmount: function() {
        ui.destroyTabs('inventory_items_window_tabs');
      }
    };

    inventory_items_window.start();
  </script>

  <div class='resize-handle'></div>
</div>