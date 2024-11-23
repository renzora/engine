<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>

<div data-window="ui_overlay_window" data-close="false">
  <div class="w-auto fixed top-2 right-2 flex flex-col justify-between items-start text-white">

  <div class="bg-gray-900 bg-opacity-80 rounded-lg p-4 shadow-lg pixel-corners">

    <!-- Top Section: Health, Energy Bars, Player Info -->
    <div class="flex flex-col space-y-4">
      
      <!-- Health and Energy Bars -->
      <div class="flex justify-between items-center space-x-8">
        <!-- Health -->
        <div class="flex items-center">
          <span class="text-xs font-medium text-gray-400 mr-2">Health</span>
          <div class="w-28 bg-gray-700 rounded h-3">
            <div id="health-bar" class="bg-red-500 h-3 rounded" style="width: 100%;"></div>
          </div>
        </div>

        <!-- Energy -->
        <div class="flex items-center">
          <span class="text-xs font-medium text-gray-400 mr-2">Energy</span>
          <div class="w-28 bg-gray-700 rounded h-3">
            <div id="energy-bar" class="bg-blue-500 h-3 rounded" style="width: 80%;"></div>
          </div>
        </div>
      </div>

      <!-- Player Info: Level and XP -->
      <div class="flex justify-between items-center space-x-8">

        <!-- XP -->
        <div class="flex items-center">
        <div class="flex items-center">
  <span class="text-xs font-medium text-gray-400 mr-2">Level</span>
  <span id="player-level" class="text-xs font-semibold text-green-400 mr-2">10</span>
</div>
          <div class="w-28 bg-gray-700 rounded h-3">
            <div id="xp-bar" class="bg-purple-500 h-3 rounded" style="width: 60%;"></div>
          </div>
        </div>
      </div>
    </div>

    <!-- Middle Section: Bullet Counter and Reload Meter -->
    <div class="flex items-center justify-between space-x-8 mt-4">
      <!-- Reload Meter and Counter -->
      <div class="relative flex items-center w-32">
        <div class="absolute inset-0 rounded-md z-0 h-full flex items-center">
          <div id="reload-meter" class="bg-green-600 h-full rounded-md" style="width: 0%;"></div>
        </div>
        <span id="bullet-round-counter" class="text-xs font-semibold text-gray-200 relative z-10 px-2">80/13</span>
      </div>

      <!-- Bullet Container -->
      <div id="bullet-container" class="flex space-x-1">
        <?php for ($i = 0; $i < 10; $i++): ?>
          <span class="bullet h-5 w-2 bg-gray-600 rounded-b-md"></span>
        <?php endfor; ?>
      </div>
    </div>

    <!-- Bottom Section: Game Time, Coins, and R3 Button -->
    <div class="flex items-center justify-between space-x-8 mt-4">
      <!-- Game Time -->
      <span id="game_time" class="text-xs font-medium text-gray-400">12:34</span>

      <!-- Coins -->
      <div class="flex items-center">
        <span class="text-xs font-medium text-gray-400 mr-2">Coins:</span>
        <span id="player-coins" class="text-xs font-semibold text-yellow-400">100</span>
      </div>

      <!-- R3 Button -->
      <button id="toggle-submenu" class="bg-gray-700 text-white text-xs rounded-md px-2 py-1">R3</button>
    </div>

    <!-- Submenu Section (Hidden by Default) -->
    <div id="submenu" class="max-h-0 overflow-hidden transition-all duration-300 ease-in-out mt-2 w-full">

      <!-- Submenu Content -->
      <div class="flex flex-col space-y-4 mt-4">

        <!-- Stats Section -->
        <div class="bg-gray-800 p-3 rounded-md">
          <span class="text-xs font-medium text-gray-300">Player Stats</span>
          <div class="flex flex-col space-y-2 mt-2">
            <div class="flex items-center space-x-2">
              <span class="text-xs font-medium text-gray-400">Strength:</span>
              <span class="text-xs font-semibold text-white">15</span>
            </div>
            <div class="flex items-center space-x-2">
              <span class="text-xs font-medium text-gray-400">Agility:</span>
              <span class="text-xs font-semibold text-white">12</span>
            </div>
            <div class="flex items-center space-x-2">
              <span class="text-xs font-medium text-gray-400">Intelligence:</span>
              <span class="text-xs font-semibold text-white">18</span>
            </div>
          </div>
        </div>

        <!-- Skills Section -->
        <div class="bg-gray-800 p-3 rounded-md">
          <span class="text-xs font-medium text-gray-300">Skills Progress</span>
          <div class="flex flex-col space-y-2 mt-2">
            <div class="flex items-center">
              <span class="text-xs font-medium text-gray-400">Sword Mastery</span>
              <div class="w-28 bg-gray-700 rounded-full h-3 ml-2">
                <div id="sword-skill" class="bg-red-500 h-3 rounded-full" style="width: 80%;"></div>
              </div>
            </div>
            <div class="flex items-center">
              <span class="text-xs font-medium text-gray-400">Archery</span>
              <div class="w-28 bg-gray-700 rounded-full h-3 ml-2">
                <div id="archery-skill" class="bg-green-500 h-3 rounded-full" style="width: 60%;"></div>
              </div>
            </div>
            <div class="flex items-center">
              <span class="text-xs font-medium text-gray-400">Magic</span>
              <div class="w-28 bg-gray-700 rounded-full h-3 ml-2">
                <div id="magic-skill" class="bg-blue-500 h-3 rounded-full" style="width: 90%;"></div>
              </div>
            </div>
          </div>
        </div>

        <!-- Inventory Section -->
        <div class="bg-gray-800 p-3 rounded-md">
          <span class="text-xs font-medium text-gray-300">Inventory</span>
          <div class="flex flex-col space-y-2 mt-2">
            <div class="flex items-center space-x-2">
              <span class="text-xs font-medium text-gray-400">Potions:</span>
              <span class="text-xs font-semibold text-white">3</span>
            </div>
            <div class="flex items-center space-x-2">
              <span class="text-xs font-medium text-gray-400">Weapons:</span>
              <span class="text-xs font-semibold text-white">2</span>
            </div>
            <div class="flex items-center space-x-2">
              <span class="text-xs font-medium text-gray-400">Armor:</span>
              <span class="text-xs font-semibold text-white">1</span>
            </div>
          </div>
        </div>

        <!-- Next Level Progress Section -->
        <div class="bg-gray-800 p-3 rounded-md">
          <span class="text-xs font-medium text-gray-300">Next Level</span>
          <div class="flex items-center mt-2">
            <span class="text-xs font-medium text-gray-400">XP to Level Up:</span>
            <div class="w-28 bg-gray-700 rounded-full h-3 ml-2">
              <div id="level-up-progress" class="bg-purple-500 h-3 rounded-full" style="width: 50%;"></div>
            </div>
          </div>
        </div>

        <!-- Other Section -->
        <div class="bg-gray-800 p-3 rounded-md">
          <span class="text-xs font-medium text-gray-300">Other</span>
          <div class="flex items-center space-x-2 mt-2">
            <span class="text-xs font-medium text-gray-400">Quests:</span>
            <span class="text-xs font-semibold text-white">5</span>
          </div>
          <div class="flex items-center space-x-2">
            <span class="text-xs font-medium text-gray-400">Achievements:</span>
            <span class="text-xs font-semibold text-white">10</span>
          </div>
        </div>

      </div>
    </div>
  </div>

        </div>

  <script>
    var ui_overlay_window = {
      bulletsPerRound: 80,
      remainingBullets: 80,
      remainingRounds: 13,
      isReloading: false,
      justReloaded: false,
      reloadProgress: 0,
      reloadInterval: null,
      reloadTime: 1000,

      start: function() {
        this.updateBulletRoundCounter();
        this.updateBullets(this.remainingBullets);
        this.updateReloadMeter(0);
        this.updatePlayerStats(); // Update player stats like level and XP
        this.initializeMenuToggle(); // Initialize the menu toggle
      },

      initializeMenuToggle: function() {
        const toggleButton = document.getElementById('toggle-submenu');
        const submenu = document.getElementById('submenu');

        toggleButton.addEventListener('click', function() {
          submenu.classList.toggle('max-h-0');
          submenu.classList.toggle('max-h-[500px]');
        });
      },

      updateBullets: function(remaining) {
        const bullets = document.querySelectorAll('#bullet-container > span');
        this.remainingBullets = remaining;

        bullets.forEach((bullet, index) => {
          bullet.classList.remove('bg-gray-600', 'bg-gradient-to-r', 'from-yellow-500', 'via-yellow-400', 'to-yellow-300', 'from-orange-500', 'via-orange-400', 'to-orange-300', 'from-red-500', 'via-red-400', 'to-red-300');

          if (index < remaining) {
            if (remaining <= 3) {
              bullet.classList.add('bg-gradient-to-r', 'from-red-500', 'via-red-400', 'to-red-300');
            } else if (remaining <= 7) {
              bullet.classList.add('bg-gradient-to-r', 'from-orange-500', 'via-orange-400', 'to-orange-300');
            } else {
              bullet.classList.add('bg-gradient-to-r', 'from-yellow-500', 'via-yellow-400', 'to-yellow-300');
            }
          } else {
            bullet.classList.add('bg-gray-600');
          }
        });

        this.updateBulletRoundCounter();

        if (this.remainingBullets === 0 && this.remainingRounds > 0) {
          console.log("Press and hold 'X' to reload the next round.");
        } else if (this.remainingBullets === 0 && this.remainingRounds <= 0) {
          this.noBulletsLeft();
        }
      },

      updateBulletRoundCounter: function() {
        document.getElementById('bullet-round-counter').textContent = `${this.remainingBullets}/${this.remainingRounds}`;
      },

      noBulletsLeft: function() {
        console.log("Out of bullets and rounds!");
      },

      handleReload: function() {
        if (this.remainingBullets <= 0 && this.remainingRounds > 0) {
          this.startReloading();
        } else if (this.remainingBullets < this.bulletsPerRound && this.remainingRounds > 0) {
          this.startReloading();
        } else {
          console.log("Cannot reload, either bullets are full or no rounds left.");
        }
      },

      startReloading: function() {
        if (this.isReloading || this.remainingRounds <= 0) return;

        this.isReloading = true;
        this.reloadProgress = 0;
        const intervalTime = 100;
        const incrementPerInterval = intervalTime / this.reloadTime;

        this.reloadInterval = setInterval(() => {
          this.reloadProgress += incrementPerInterval;
          this.updateReloadMeter(this.reloadProgress);
          if (this.reloadProgress >= 1) {
            this.completeReload();
          }
        }, intervalTime);
      },

      stopReloading: function() {
        if (this.reloadInterval) {
          clearInterval(this.reloadInterval);
          this.reloadInterval = null;
          this.reloadProgress = 0;
          this.updateReloadMeter(this.reloadProgress);
          this.isReloading = false;
        }
      },

      completeReload: function() {
        this.stopReloading();
        this.nextRound();
        console.log("Reload complete!");
        audio.playAudio("reload_gun", assets.use('reload_gun'), 'sfx', false);
        this.isReloading = false;
        this.justReloaded = true;
      },

      updateReloadMeter: function(progress) {
        const reloadMeter = document.getElementById('reload-meter');
        if (reloadMeter) {
          reloadMeter.style.width = `${progress * 100}%`;
        }
      },

      nextRound: function() {
        if (this.remainingRounds > 0) {
          this.remainingRounds -= 1;
          this.remainingBullets = this.bulletsPerRound;
          this.updateBulletRoundCounter();
          this.updateBullets(this.remainingBullets);
        }
      },

      updatePlayerStats: function() {
        document.getElementById('player-level').textContent = "10"; // Example level
        document.getElementById('xp-bar').style.width = "60%"; // Example XP bar width
      }
    };

    ui_overlay_window.start();
  </script>
</div>

<?php
}
?>
