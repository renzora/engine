<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>

<div data-window="ui_overlay_window" data-close="false">
  <div class="w-auto mx-auto fixed top-0 left-1/2 transform -translate-x-1/2 flex flex-col justify-between items-center bg-gray-900/80 text-white rounded-b-md shadow-lg overflow-hidden px-4 py-2">

    <!-- Top Section: Health, Energy Bars, Player Info, and R3 Button -->
    <div class="flex items-center justify-between w-full space-x-4">
      <div class="flex items-center space-x-4">
        <div class="flex items-center">
          <span class="text-sm font-medium text-gray-400 mr-2">Health</span>
          <div class="w-28 bg-gray-800 rounded-full h-2">
            <div id="health-bar" class="bg-red-500 h-2 rounded-full"></div>
          </div>
        </div>
        <div class="flex items-center">
          <span class="text-sm font-medium text-gray-400 mr-2">Energy</span>
          <div class="w-28 bg-gray-800 rounded-full h-2">
            <div id="energy-bar" class="bg-blue-500 h-2 rounded-full"></div>
          </div>
        </div>

        <!-- Player Info: Level, XP -->
        <div class="flex items-center">
          <span class="text-sm font-medium text-gray-400 mr-2">Level:</span>
          <span id="player-level" class="text-sm font-semibold text-green-400">10</span>
        </div>
        <div class="flex items-center">
          <span class="text-sm font-medium text-gray-400 mr-2">XP:</span>
          <div class="w-28 bg-gray-800 rounded-full h-2">
            <div id="xp-bar" class="bg-purple-500 h-2 rounded-full" style="width: 60%;"></div>
          </div>
        </div>
      </div>

      <!-- Center Section: Bullet Counter with Reload Meter -->
      <div class="flex items-center space-x-4 relative">
        <div class="relative flex items-center">
          <div class="absolute inset-0 rounded-md z-0 h-full flex items-center">
            <div id="reload-meter" class="bg-green-600 h-full rounded-md" style="width: 0%;"></div>
          </div>
          <span id="bullet-round-counter" class="text-sm font-semibold text-gray-200 relative z-10 px-2"></span>
        </div>
        <div id="bullet-container" class="flex z-10 relative space-x-1">
          <?php for ($i = 0; $i < 10; $i++): ?>
            <span class="bullet h-5 w-2 shadow-inner rounded-b-md bg-gray-600"></span>
          <?php endfor; ?>
        </div>
      </div>

      <!-- Right Section: Game Time, Coins, and R3 Button -->
      <div class="flex items-center space-x-4">
        <span id="game_time" class="text-sm font-medium text-gray-400 ml-4"></span>
        <div class="flex items-center">
          <span class="text-sm font-medium text-gray-400 mr-2">Coins:</span>
          <span id="player-coins" class="text-sm font-semibold text-yellow-400">100</span>
        </div>

<button id="toggle-submenu" class="bg-gray-700 text-white text-sm rounded-md px-1">R3</button>

      </div>
    </div>

    <!-- Submenu Section with Divider (Hidden by Default) -->
    <div id="submenu" class="max-h-0 overflow-hidden transition-all duration-300 ease-in-out">

      <!-- Divider (part of the submenu) -->
      <div class="w-full border-b border-gray-700 pt-2 mb-2"></div>

      <!-- Submenu Content as Columns -->
      <div class="flex space-x-8">

        <!-- Stats Section -->
        <div class="flex flex-col space-y-2 bg-gray-800 p-4 rounded-md w-1/5">
          <span class="text-sm font-medium text-gray-300">Player Stats</span>
          <div class="flex items-center space-x-2">
            <span class="text-sm font-medium text-gray-400">Strength:</span>
            <span class="text-sm font-semibold text-white">15</span>
          </div>
          <div class="flex items-center space-x-2">
            <span class="text-sm font-medium text-gray-400">Agility:</span>
            <span class="text-sm font-semibold text-white">12</span>
          </div>
          <div class="flex items-center space-x-2">
            <span class="text-sm font-medium text-gray-400">Intelligence:</span>
            <span class="text-sm font-semibold text-white">18</span>
          </div>
        </div>

        <!-- Skills Section -->
        <div class="flex flex-col space-y-2 bg-gray-800 p-4 rounded-md w-1/5">
          <span class="text-sm font-medium text-gray-300">Skills Progress</span>
          <div class="flex items-center">
            <span class="text-sm font-medium text-gray-400">Sword Mastery</span>
            <div class="w-28 bg-gray-700 rounded-full h-2 ml-2">
              <div id="sword-skill" class="bg-red-500 h-2 rounded-full" style="width: 80%;"></div>
            </div>
          </div>
          <div class="flex items-center">
            <span class="text-sm font-medium text-gray-400">Archery</span>
            <div class="w-28 bg-gray-700 rounded-full h-2 ml-2">
              <div id="archery-skill" class="bg-green-500 h-2 rounded-full" style="width: 60%;"></div>
            </div>
          </div>
          <div class="flex items-center">
            <span class="text-sm font-medium text-gray-400">Magic</span>
            <div class="w-28 bg-gray-700 rounded-full h-2 ml-2">
              <div id="magic-skill" class="bg-blue-500 h-2 rounded-full" style="width: 90%;"></div>
            </div>
          </div>
        </div>

        <!-- Inventory Section -->
        <div class="flex flex-col space-y-2 bg-gray-800 p-4 rounded-md w-1/5">
          <span class="text-sm font-medium text-gray-300">Inventory</span>
          <div class="flex items-center space-x-2">
            <span class="text-sm font-medium text-gray-400">Potions:</span>
            <span class="text-sm font-semibold text-white">3</span>
          </div>
          <div class="flex items-center space-x-2">
            <span class="text-sm font-medium text-gray-400">Weapons:</span>
            <span class="text-sm font-semibold text-white">2</span>
          </div>
          <div class="flex items-center space-x-2">
            <span class="text-sm font-medium text-gray-400">Armor:</span>
            <span class="text-sm font-semibold text-white">1</span>
          </div>
        </div>

        <!-- Next Level Progress Section -->
        <div class="flex flex-col space-y-2 bg-gray-800 p-4 rounded-md w-1/5">
          <span class="text-sm font-medium text-gray-300">Next Level</span>
          <div class="flex items-center">
            <span class="text-sm font-medium text-gray-400">XP to Level Up:</span>
            <div class="w-28 bg-gray-700 rounded-full h-2 ml-2">
              <div id="level-up-progress" class="bg-purple-500 h-2 rounded-full" style="width: 50%;"></div>
            </div>
          </div>
        </div>

        <!-- Other Section -->
        <div class="flex flex-col space-y-2 bg-gray-800 p-4 rounded-md w-1/5">
          <span class="text-sm font-medium text-gray-300">Other</span>
          <div class="flex items-center space-x-2">
            <span class="text-sm font-medium text-gray-400">Quests:</span>
            <span class="text-sm font-semibold text-white">5</span>
          </div>
          <div class="flex items-center space-x-2">
            <span class="text-sm font-medium text-gray-400">Achievements:</span>
            <span class="text-sm font-semibold text-white">10</span>
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
    // Toggle between max-h-0 (collapsed) and max-h-[500px] (expanded)
    submenu.classList.toggle('max-h-0');
    submenu.classList.toggle('max-h-[500px]'); // Adjust the height as per your submenu content
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
        audio.playAudio("reload_gun", assets.load('reload_gun'), 'sfx', false);
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

      // Update player stats (level, XP)
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
