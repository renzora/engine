<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
<div data-window="ui_overlay_window" data-close="false">
  
  <!-- Avatar and Rings fixed at top center -->
  <div class="fixed top-4 left-1/2 transform -translate-x-1/2 flex flex-col items-center space-y-2 text-white z-50">
    <!-- Top Section: Avatar -->
    <div class="flex items-center space-x-6 p-4 relative">
      <!-- Avatar Container -->
      <div class="relative w-16 h-16">
        <!-- Avatar Image with Grey Border -->
        <img
          src="assets/img/sprites/portrait/lady_01.png"
          alt="Player Avatar"
          class="rounded-full w-full h-full object-cover border-4 border-transparent"
        />

        <!-- Rings Container -->
        <div class="absolute inset-0 flex items-center justify-center">
          <!-- Energy Ring -->
          <div class="absolute" style="width: 60px; height: 60px;">
            <svg class="w-full h-full" viewBox="0 0 60 60">
              <circle
                class="progress-ring progress-ring-energy stroke-blue-500"
                stroke-width="5"
                fill="none"
                r="27"
                cx="30"
                cy="30"
              />
            </svg>
          </div>
          <!-- Health Ring -->
          <div class="absolute" style="width: 76px; height: 76px;">
            <svg class="w-full h-full" viewBox="0 0 76 76">
              <circle
                class="progress-ring progress-ring-health stroke-red-500"
                stroke-width="5"
                fill="none"
                r="33"
                cx="38"
                cy="38"
              />
            </svg>
          </div>
          <!-- XP Ring -->
          <div class="absolute" style="width: 78px; height: 78px;">
            <svg class="w-full h-full" viewBox="0 0 78 78">
              <circle
                class="progress-ring progress-ring-xp stroke-white-500"
                stroke-width="6"
                fill="none"
                r="36"
                cx="39"
                cy="39"
              />
            </svg>
          </div>
        </div>
      </div>
      
      <!-- Removed Player Username -->
      <!--
      <div class="flex flex-col justify-center">
        <span class="text-xl font-bold text-yellow-400">Player123</span>
        <span class="text-sm font-medium text-gray-400">
          Coins: <span id="player-coins" class="text-yellow-400">100</span> |
          <span id="game_time" class="text-lg font-medium text-white">00:00</span>
        </span>
      </div>
      -->
    </div>
  </div>

  <!-- Existing menu fixed at top right -->
  <div class="fixed bottom-4 right-4 flex flex-col space-y-4 text-white z-10">

      <!-- Gun HUD: Bullets, Reload Meter, and Bullet Counter -->
      <div class="bg-gray-900 bg-opacity-40 fog pixel-corners p-4 rounded-lg shadow-md">
      <div class="flex items-center justify-between space-x-4">
        <!-- Reload Meter -->
        <div class="flex flex-col items-start space-y-1">
          <span class="text-xs text-gray-400">Reload</span>
          <div class="relative w-40 h-4 bg-gray-600 rounded-full overflow-hidden">
            <div id="reload-meter" class="absolute h-full bg-green-500 rounded-full" style="width: 0%;"></div>
          </div>
        </div>

        <!-- Bullet Counter -->
        <div class="text-center">
          <span id="bullet-round-counter" class="text-lg font-bold text-gray-200">80/13</span>
        </div>
      </div>
    </div>

    <!-- Moved Coins and Time to the Right Menu -->
    <div class="bg-gray-900 bg-opacity-40 fog pixel-corners p-4 rounded-lg shadow-md">
      <div class="flex flex-col space-y-2">
        <div class="flex justify-between items-center">
          <span class="text-xs text-gray-400">Coins:</span>
          <span id="player-coins" class="text-yellow-400">100</span>
        </div>
        <div class="flex justify-between items-center">
          <span class="text-xs text-gray-400">Time:</span>
          <!-- Updated Time Span: White and Larger Text -->
          <span id="game_time" class="text-lg font-medium text-white">00:00</span>
        </div>
      </div>
    </div>

    <button onclick="effects.bubbleEffect.create(game.sprites[game.playerid], '#FFFF00');">Bubbles!</button>

    <!-- Submenu Toggle -->
    <button
      id="toggle-submenu"
      class="bg-gray-900 bg-opacity-40 fog pixel-corners hover:bg-gray-600 text-white text-sm font-medium rounded-md px-4 py-2 shadow-md"
      aria-expanded="false"
      aria-controls="submenu"
    >
      Submenu
    </button>

    <!-- Submenu -->
    <div
      id="submenu"
      class="max-h-0 p-0 overflow-hidden bg-gray-900 bg-opacity-40 fog pixel-corners rounded-lg transition-all duration-300 ease-in-out shadow-md"
    >
      <div class="p-4">
        <div class="flex flex-col space-y-4">
          <!-- Player Stats -->
          <div>
            <span class="text-sm font-medium text-gray-300">Stats</span>
            <div class="grid grid-cols-3 gap-4 text-xs font-medium text-gray-400 mt-2">
              <div>Strength: <span class="text-white">15</span></div>
              <div>Agility: <span class="text-white">12</span></div>
              <div>Intelligence: <span class="text-white">18</span></div>
            </div>
          </div>

          <!-- Skills -->
          <div>
            <span class="text-sm font-medium text-gray-300">Skills</span>
            <div class="space-y-2 text-xs font-medium text-gray-400 mt-2">
              <div>
                Sword Mastery
                <div class="w-full bg-gray-700 rounded-full h-3 mt-1">
                  <div class="bg-red-500 h-3 rounded-full" style="width: 80%;"></div>
                </div>
              </div>
              <div>
                Archery
                <div class="w-full bg-gray-700 rounded-full h-3 mt-1">
                  <div class="bg-green-500 h-3 rounded-full" style="width: 60%;"></div>
                </div>
              </div>
              <div>
                Magic
                <div class="w-full bg-gray-700 rounded-full h-3 mt-1">
                  <div class="bg-blue-500 h-3 rounded-full" style="width: 90%;"></div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div> <!-- End of inner p-4 div -->
    </div>

  </div>

  <style>
    .fog {
      backdrop-filter: blur(5px) brightness(0.5); /* Apply fogging effect */
      -webkit-backdrop-filter: blur(5px) brightness(0.5); /* For Safari */
    }
    .progress-ring {
      stroke-linecap: round;
      transition: stroke-dashoffset 0.5s ease-in-out;
 
    }

    /* Specific Shadows for Each Ring */
    .progress-ring-health {
      stroke-dasharray: 214; /* Rounded from 213.63 */
      stroke-dashoffset: 214;
      filter: drop-shadow(0 0 4px rgba(255, 0, 0, 0.7)) /* Red glow */
              drop-shadow(0 0 6px rgba(0, 0, 0, 0.5)); /* Dark outer shadow */
    }

    .progress-ring-energy {
      stroke-dasharray: 163; /* Rounded from 163.36 */
      stroke-dashoffset: 163;
      filter: drop-shadow(0 0 4px rgba(0, 0, 255, 0.7)) /* Blue glow */
              drop-shadow(0 0 6px rgba(0, 0, 0, 0.5)); /* Dark outer shadow */
    }

    .progress-ring-xp {
      stroke-dasharray: 226; /* Rounded from 226.19 */
      stroke-dashoffset: 226;
      filter: drop-shadow(0 0 4px rgba(255, 255, 255, 0.7)) /* White glow */
              drop-shadow(0 0 6px rgba(0, 0, 0, 0.5)); /* Dark outer shadow */
    }

    .progress-ring-health,
    .progress-ring-energy,
    .progress-ring-xp {
      transform: rotate(-90deg); /* Start progress from top */
      transform-origin: 50% 50%;
    }

    /* Additional Styling for Player Info */
    /* Removed margin-left since username is removed */
    #player-coins,
    #game_time {
      margin-left: 2px;
    }

    /* Optional: Improve readability */
    .text-gray-400 {
      line-height: 1.5;
    }
  </style>

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
        //this.updatePlayerStats(); // Update player stats like level and XP
        this.initializeMenuToggle(); // Initialize the menu toggle
        this.updateXPRing(368, 500); // Example XP values
        this.updateHealthRing(57, 100); // Example health values
        this.updateEnergyRing(79, 100);
      },

      initializeMenuToggle: function() {
        const toggleButton = document.getElementById('toggle-submenu');
        const submenu = document.getElementById('submenu');

        toggleButton.addEventListener('click', () => {
          const isExpanded = toggleButton.getAttribute('aria-expanded') === 'true';
          toggleButton.setAttribute('aria-expanded', !isExpanded);
          
          submenu.classList.toggle('max-h-0');
          submenu.classList.toggle('max-h-[500px]');
          submenu.classList.toggle('p-0');
          submenu.classList.toggle('p-4');
        });
      },

      updateBullets: function(remaining) {
        const bullets = document.querySelectorAll('#bullet-container > span');
        this.remainingBullets = remaining;

        bullets.forEach((bullet, index) => {
          bullet.classList.remove(
            'bg-gray-600', 
            'bg-gradient-to-r', 
            'from-yellow-500', 
            'via-yellow-400', 
            'to-yellow-300', 
            'from-orange-500', 
            'via-orange-400', 
            'to-orange-300', 
            'from-red-500', 
            'via-red-400', 
            'to-red-300'
          );

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
      },

      updateHealthRing: function(currentHealth, maxHealth) {
        const circumference = 214; // Correct circumference for health ring
        const percentage = (currentHealth / maxHealth) * 100;
        const offset = circumference - (circumference * percentage) / 100;
        document.querySelector('.progress-ring-health').style.strokeDashoffset = offset;
      },

      updateEnergyRing: function(currentEnergy, maxEnergy) {
        const circumference = 163; // Correct circumference for energy ring
        const percentage = (currentEnergy / maxEnergy) * 100;
        const offset = circumference - (circumference * percentage) / 100;
        document.querySelector('.progress-ring-energy').style.strokeDashoffset = offset;
      },

      updateXPRing: function(currentXP, maxXP) {
        const circumference = 226; // Correct circumference for XP ring
        const percentage = (currentXP / maxXP) * 100;
        const offset = circumference - (circumference * percentage) / 100;
        document.querySelector('.progress-ring-xp').style.strokeDashoffset = offset;
      }
    };

    ui_overlay_window.start();
  </script>
</div>

<?php
}
?>
