<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>

<div data-window="ui_overlay_window" data-close="false">
  <div class="w-1/3 mx-auto fixed top-0 left-1/2 transform -translate-x-1/2 flex justify-between items-center bg-gray-900/90 text-white rounded-b-md shadow-lg overflow-hidden p-2">

    <!-- Left Section: Health & Energy Bars -->
    <div class="flex items-center space-x-6">
      <div class="flex items-center">
        <span class="text-sm font-medium text-gray-400 mr-1">Health</span>
        <div class="w-28 bg-gray-800 rounded-full h-2">
          <div id="health-bar" class="bg-red-500 h-2 rounded-full"></div>
        </div>
      </div>
      <div class="flex items-center">
        <span class="text-sm font-medium text-gray-400 mr-1">Energy</span>
        <div class="w-28 bg-gray-800 rounded-full h-2">
          <div id="energy-bar" class="bg-blue-500 h-2 rounded-full"></div>
        </div>
      </div>
    </div>

    <!-- Center Section: Bullet Counter with Reload Meter -->
    <div class="flex items-center space-x-2 relative">
      <!-- Bullet Counter Container with Reload Meter Behind the Text -->
      <div class="relative flex items-center">
        <!-- Green Reload Meter behind text only -->
        <div class="absolute inset-0 rounded-md z-0 h-full flex items-center">
          <div id="reload-meter" class="bg-green-600 h-full rounded-md" style="width: 0%;"></div>
        </div>

        <!-- Bullet Counter Text -->
        <span id="bullet-round-counter" class="text-sm font-semibold text-gray-200 relative z-10 px-2"></span>
      </div>

      <!-- Bullet Container -->
      <div id="bullet-container" class="flex z-10 relative">
        <!-- Bullet spans -->
        <?php for ($i = 0; $i < 10; $i++): ?>
          <span class="bullet h-5 w-2 shadow-inner rounded-b-md"></span>
        <?php endfor; ?>
      </div>
    </div>

    <!-- Right Section: Game Time and Coins -->
    <div class="flex items-center space-x-2">
      <span id="game_time" class="text-sm font-medium text-gray-400"></span>
      <div class="flex items-center">
        <span class="text-sm font-medium text-gray-400 mr-1">Coins:</span>
        <span id="player-coins" class="text-sm font-semibold text-yellow-400">100</span>
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
      }
    };

    ui_overlay_window.start();
  </script>
</div>

<?php
}
?>