<div data-window="ui_overlay_window" data-close="false">
  <div id="ui_overlay_window" class="w-80 fixed top-4 right-4 z-10 flex flex-col bg-gray-900/80 text-white rounded-md shadow-lg overflow-hidden">
    <!-- Top Section with Time and Coins -->
    <div class="bg-gray-800 flex items-center justify-between rounded-t-md">
      <span id="game_time" class="text-sm font-medium text-gray-400 p-3">00:00</span>
      <div class="flex items-center p-3">
        <span class="text-xs font-medium text-gray-400 mr-1">Coins:</span>
        <span id="player-coins" class="text-sm font-semibold text-yellow-400">100</span>
      </div>
    </div>

    <!-- Health & Energy Bars -->
    <div class="p-3">
      <div class="flex items-center justify-between mb-1">
        <span class="text-xs font-medium text-gray-400">Health</span>
        <span class="text-xs font-medium text-gray-400">Energy</span>
      </div>
      <div class="flex space-x-2">
        <div class="w-1/2 bg-gray-800 rounded-full h-2">
          <div id="health-bar" class="bg-red-500 h-2 rounded-full" style="width: 75%;"></div>
        </div>
        <div class="w-1/2 bg-gray-800 rounded-full h-2">
          <div id="energy-bar" class="bg-blue-500 h-2 rounded-full" style="width: 50%;"></div>
        </div>
      </div>
    </div>

    <!-- Bullet Counter with Reload Meter as Background -->
    <div class="relative m-1">
      <div class="absolute inset-0 bg-gray-800 h-full w-full overflow-hidden rounded-md">
        <div id="reload-meter" class="bg-green-600 h-full w-full rounded-md" style="width: 0%;"></div>
      </div>
      <div class="relative flex items-center justify-between p-2">
        <!-- Combined Bullets and Rounds Counter -->
        <span id="bullet-round-counter" class="text-sm font-semibold text-gray-200 mr-2"></span>
        <div id="bullet-container" class="flex flex-wrap gap-1">
          <!-- Bullet spans -->
          <span class="bullet h-5 w-2.5 shadow-inner rounded-b-md"></span>
          <span class="bullet h-5 w-2.5 shadow-inner rounded-b-md"></span>
          <span class="bullet h-5 w-2.5 shadow-inner rounded-b-md"></span>
          <span class="bullet h-5 w-2.5 shadow-inner rounded-b-md"></span>
          <span class="bullet h-5 w-2.5 shadow-inner rounded-b-md"></span>
          <span class="bullet h-5 w-2.5 shadow-inner rounded-b-md"></span>
          <span class="bullet h-5 w-2.5 shadow-inner rounded-b-md"></span>
          <span class="bullet h-5 w-2.5 shadow-inner rounded-b-md"></span>
          <span class="bullet h-5 w-2.5 shadow-inner rounded-b-md"></span>
          <span class="bullet h-5 w-2.5 shadow-inner rounded-b-md"></span>
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
    reloadTime: 1000, // Reload time in milliseconds (e.g., 2000ms = 2 seconds)

    start: function() {
        this.updateBulletRoundCounter();
        this.updateBullets(this.remainingBullets);
        this.updateReloadMeter(0); // Initialize reload meter
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
        const intervalTime = 100; // Interval time in milliseconds
        const incrementPerInterval = intervalTime / this.reloadTime; // Calculate the progress increment for each interval

        this.reloadInterval = setInterval(() => {
            this.reloadProgress += incrementPerInterval;
            this.updateReloadMeter(this.reloadProgress);
            if (this.reloadProgress >= 1) {
                this.completeReload();  // Call completeReload when done
            }
        }, intervalTime);
    },

stopReloading: function() {
    if (this.reloadInterval) {
        clearInterval(this.reloadInterval);
        this.reloadInterval = null;
        this.reloadProgress = 0;
        this.updateReloadMeter(this.reloadProgress);
        this.isReloading = false; // Ensure isReloading is set to false
    }
},

completeReload: function() {
        console.log("Completing reload...");
        this.stopReloading();  // Ensure the reload interval is cleared
        this.nextRound();
        console.log("Reload complete!");
        audio.playAudio("reload_gun", assets.load('reload_gun'), 'sfx', false);
        this.isReloading = false;  // Reset isReloading flag
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
            console.log("Reloaded!");
        }
    }
    };

    ui_overlay_window.start();
  </script>
</div>
