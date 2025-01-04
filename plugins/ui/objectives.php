<div data-window='ui_objectives_window' data-close="false">
  <div id="ui_objectives_window" class="w-72 fixed top-1/4 right-2 z-10 flex rounded flex-col tracking-tight">
    <div id="ui_objectives_container" class="flex flex-col space-y-1 p-2">
      <!-- Objectives will be dynamically inserted here by the displayObjectives method -->
    </div>
  </div>

  <script>
  var ui_objectives_window = {
    objectives: [
        { name: "Find the hidden sword", status: false },
        { name: "Plant the apple seeds in renzora Garden", status: false },
        { name: "Sell gold at oakenbridge Market", status: false },
        { name: "Find the hidden sword", status: true },
        { name: "Find the hidden sword", status: true },
        { name: "Defeat the dragon", status: true },
        { name: "Collect 100 coins from merchant", status: false }
    ],
    displayObjectives: function() {
      const objectivesContainer = document.getElementById('ui_objectives_container');
      if (objectivesContainer) {
        objectivesContainer.innerHTML = '';
        this.objectives.forEach(obj => {
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
    }
  };

  ui_objectives_window.displayObjectives();
  </script>

  <style>

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
</div>
