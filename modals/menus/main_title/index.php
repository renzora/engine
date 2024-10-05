<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
?>
<div data-window='main_title_window' class='fixed inset-0 bg-black bg-opacity-40 flex overflow-hidden'>

<!-- Glitch Effect Overlay -->
<div class='absolute inset-0 z-0'>
  <div class="glitch-overlay"></div>
  <div class="color-shift"></div>
</div>

<!-- Left side with large text menu -->
<div class='relative z-10 w-full lg:w-1/3 h-full flex flex-col justify-center pl-8 pr-4 lg:pl-32 lg:pr-12'>
  <div class="season-info mb-8 lg:mb-16">
    <div class="season-title text-xl md:text-4xl lg:text-6xl font-extrabold text-white tracking-tighter drop-shadow-lg">
      Renzora Season 1
    </div>

    <!-- Countdown element right below the season title -->
    <div id="countdown" class="countdown-timer text-yellow-500 text-lg md:text-xl lg:text-2xl font-extrabold mt-2">
      <!-- Countdown text will be placed here -->
    </div>
  </div>

  <!-- Menu Items -->
  <div id="story-mode-tab" class="menu-item text-4xl md:text-5xl lg:text-6xl font-extrabold text-white mb-6 md:mb-8 lg:mb-10 cursor-pointer hover:text-yellow-500 tracking-tighter drop-shadow-lg">
    Story Mode
  </div>
  <div id="online-tab" class="menu-item text-4xl md:text-5xl lg:text-6xl font-extrabold text-white mb-6 md:mb-8 lg:mb-10 cursor-pointer hover:text-yellow-500 tracking-tighter drop-shadow-lg">
    Online
  </div>

  <div id="community-tab" class="menu-item text-4xl md:text-5xl lg:text-6xl font-extrabold text-white mb-6 md:mb-8 lg:mb-10 cursor-pointer hover:text-yellow-500 tracking-tighter drop-shadow-lg">
    Store
  </div>
  
  <div id="settings-tab" class="menu-item text-4xl md:text-5xl lg:text-6xl font-extrabold text-white cursor-pointer hover:text-yellow-500 tracking-tighter drop-shadow-lg">
    Settings
  </div>

</div>

<!-- Bottom Left Controls -->
<div class="absolute bottom-4 left-4 text-white text-lg lg:text-2xl font-semibold tracking-wider">
  (A) select   (B) back
</div>

</div>

<style>
  /* Glitch Effect Overlay */
  .glitch-overlay {
    position: fixed;
    top: -2%;
    left: -2%;
    width: 104vw;
    height: 104vh;
    background: repeating-linear-gradient(
      45deg,
      rgba(255, 255, 255, 0.05) 0%,
      rgba(255, 255, 255, 0.05) 4%,
      transparent 4%,
      transparent 8%
    );
    opacity: 0.15;
    animation: glitchMove 15s linear infinite;
    pointer-events: none;
  }

  @keyframes glitchMove {
    0% {
      transform: translate(0, 0);
    }
    25% {
      transform: translate(-30px, -30px);
    }
    50% {
      transform: translate(40px, 40px);
    }
    75% {
      transform: translate(-45px, 35px);
    }
    100% {
      transform: translate(0, 0);
    }
  }

  /* Color Shift Background */
  .color-shift {
    position: fixed;
    top: -2%;
    left: -2%;
    width: 104vw;
    height: 104vh;
    background: linear-gradient(135deg, #ff0066, #00f0ff);
    opacity: 0.1;
    animation: colorShift 30s linear infinite;
    pointer-events: none;
  }

  /* Menu item and fly-in animations */
  .menu-item {
    position: relative;
    transition: color 0.3s ease, text-shadow 0.3s ease;
    opacity: 0;
    transform: translateX(-100%);
    pointer-events: auto;
    text-shadow: 2px 2px 5px rgba(0, 0, 0, 0.7);
  }

  .fly-in {
    opacity: 1;
    transform: translateX(0);
    transition: transform 0.5s ease-out, opacity 0.5s ease-out;
  }

  /* Ensure countdown is placed right below the title */
  .season-info {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
  }

  .countdown-timer {
    margin-top: 0.5rem; /* Small spacing below the title */
  }
</style>

<script>
var main_title_window = {
  currentMenuIndex: 0,
  countdownInterval: null,
  subMenuActive: false,
  currentSeason: 1, // Hardcoded current season
  countdownExpired: false, // Track if the countdown has expired
  countdownInitialized: false, // To ensure countdown logic runs once
  playerid: 12345, // Example player ID (can be dynamically set)

  flyInMenuItems: function () {
    const menuItems = document.querySelectorAll('.menu-item');
    menuItems.forEach((item, index) => {
      setTimeout(() => {
        item.classList.add('fly-in');
      }, index * 100);
    });
  },

  start: function () {
    this.flyInMenuItems();
    this.initMenuNavigation();
    this.initCountdown(); // Initialize the countdown when starting

    camera.panTo(350,350, 0.3);
    effects.letterboxEffect.start();
  },

  initCountdown: function () {
    if (this.countdownInitialized) return; // Only run countdown logic once
    this.countdownInitialized = true; // Mark as initialized

    const countdownElement = document.getElementById('countdown');
    const targetDate = new Date('2024-09-23T08:02:00').getTime(); // Season 2 launch date
    const now = new Date().getTime();

    // If the current date is past the target date, increment the season
    if (now >= targetDate) {
      this.currentSeason++; // Increment the season if the countdown has expired
      this.countdownExpired = true;
    }

    // Call the update season function to reflect the current or updated season in the UI
    this.updateSeasonTitle();

    // Start the countdown timer if the current time is before the target date
    if (!this.countdownExpired) {
      this.startCountdown(targetDate);
    } else {
      countdownElement.innerHTML = `Season ${this.currentSeason} is here!`;
    }
  },

  startCountdown: function (targetDate) {
    const countdownElement = document.getElementById('countdown');

    const updateCountdown = () => {
      const now = new Date().getTime();
      const timeLeft = targetDate - now;

      const days = Math.floor(timeLeft / (1000 * 60 * 60 * 24));
      const hours = Math.floor((timeLeft % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
      const minutes = Math.floor((timeLeft % (1000 * 60 * 60)) / (1000 * 60));
      const seconds = Math.floor((timeLeft % (1000 * 60)) / 1000);

      // Helper function to pad numbers with leading zeroes
      const padWithZero = (num) => (num < 10 ? `0${num}` : `${num}`);

      // Build the countdown string
      let countdownString = `Season ${this.currentSeason + 1} starts in `;
      countdownString += `${padWithZero(days)}:${padWithZero(hours)}:${padWithZero(minutes)}:${padWithZero(seconds)}`;

      if (timeLeft < 0) {
        // When the countdown finishes, show the new season
        clearInterval(this.countdownInterval);
        countdownElement.innerHTML = `Season ${this.currentSeason + 1} is here!`;
      } else {
        countdownElement.innerHTML = countdownString;
      }
    };

    // Update the countdown every second
    this.countdownInterval = setInterval(updateCountdown, 1000);
    updateCountdown(); // Run immediately on load
  },

  updateSeasonTitle: function () {
    const seasonTitle = document.querySelector('.season-title');
    seasonTitle.textContent = `Renzora Season ${this.currentSeason}`;
    this.animateSeasonChange(seasonTitle);
  },

  animateSeasonChange: function (element) {
    // Animation for the season title when it changes
    element.style.transition = "transform 0.5s ease, opacity 0.5s ease";
    element.style.transform = "scale(1.1)";
    element.style.opacity = "0.8";

    setTimeout(() => {
      element.style.transform = "scale(1)";
      element.style.opacity = "1";
    }, 500);
  },

  initMenuNavigation: function () {
    const menuItems = document.querySelectorAll('.menu-item');
    this.highlightMenuItem(this.currentMenuIndex);

    // Keyboard navigation
    document.addEventListener('keydown', (event) => {
      if (!this.subMenuActive) {
        if (event.key === 'ArrowDown') {
          this.downButton(); // Move down in the menu
        } else if (event.key === 'ArrowUp') {
          this.upButton(); // Move up in the menu
        } else if (event.key === 'Enter') {
          this.aButton(); // Press A (Enter key) to select the menu
        } else if (event.key === 'Backspace') {
          this.bButton(); // Press Circle (Backspace key) to go back
        }
      }
    });

    menuItems.forEach((item, index) => {
      item.addEventListener('mouseover', () => {
        this.currentMenuIndex = index; // Update the menu index on hover
        this.highlightMenuItem(index);
      });
    });

    // Gamepad controls (if available)
    if (typeof gamepad !== 'undefined' && gamepad.throttle) {
      this.upButton = gamepad.throttle(this.upButton.bind(this), 150);
      this.downButton = gamepad.throttle(this.downButton.bind(this), 150);
    }
  },

  aButton: function () {
    const menuItems = document.querySelectorAll('.menu-item');
    const selectedItemText = menuItems[this.currentMenuIndex].textContent.trim();

    // If the selected item is "Back", call the bButton functionality to go back
    if (selectedItemText === "Back") {
      this.bButton();
      return;
    }

    if (selectedItemText === "Online") {
      this.handleOnlineSelection(); // Trigger callback when "Online" is selected
    } else if (!this.subMenuActive && this.currentMenuIndex === 0) {
      // When in Story Mode, show New Game and Load Game
      this.showStorySubMenu();
    } else {
      console.log(`A pressed on ${selectedItemText}`);
    }
  },

  handleOnlineSelection: function () {
    // Save a value in localStorage to prevent the title screen from showing again
    localStorage.setItem('showMainTitle', 'false');

    // Callback logic when "Online" is selected
    const playerOptions = {
      id: game.playerid,
      x: 29,
      y: 23,
      isPlayer: true,
      speed: 100,
      head: 1,
      eyes: 1,
      body: 1,
      hair: 1,
      outfit: 1,
      hands: 2,
      hat: 0,
      facial: 0,
      glasses: 0,
      targetAim: false,
      maxRange: 200,
      health: 100,
      energy: 100
    };

    // Assuming `sprite` and `effects` are globally available
    sprite.create(playerOptions);
    game.mainSprite = game.sprites[game.playerid];
    game.setActiveSprite(game.playerid);
    effects.letterboxEffect.stop();
    camera.cutsceneMode = false;

    // Load other windows after selecting "Online"
    modal.close("main_title_window");
    game.modal_init();
},

  bButton: function () {
    if (this.subMenuActive) {
      this.hideStorySubMenu(); // Go back to the previous menu
    }
  },

  upButton: function () {
    this.navigateMenu(-1); // Move up in the menu
  },

  downButton: function () {
    this.navigateMenu(1); // Move down in the menu
  },

  showStorySubMenu: function () {
    this.subMenuActive = true;

    // Replace content with "New Game" and "Load Game"
    const menuContainer = document.querySelector('.relative.z-10');
    menuContainer.innerHTML = `
      <div class="text-xl md:text-4xl lg:text-6xl font-extrabold text-white mb-8 lg:mb-16 tracking-tighter drop-shadow-lg">
        Renzora Season ${this.currentSeason}
      </div>
      <div id="new-game-tab" class="menu-item text-4xl md:text-5xl lg:text-6xl font-extrabold text-white mb-6 md:mb-8 lg:mb-10 cursor-pointer hover:text-yellow-500 tracking-tighter drop-shadow-lg">
        New Game
      </div>
      <div id="load-game-tab" class="menu-item text-4xl md:text-5xl lg:text-6xl font-extrabold text-white mb-6 md:mb-8 lg:mb-10 cursor-pointer hover:text-yellow-500 tracking-tighter drop-shadow-lg">
        Load Game
      </div>
    `;

    this.flyInMenuItems(); // Fly-in effect for new submenu items
    this.currentMenuIndex = 0; // Set the first item as active
    this.highlightMenuItem(this.currentMenuIndex); // Highlight the first item
  },

  hideStorySubMenu: function () {
    this.subMenuActive = false;

    // Rebuild the original menu and trigger the fly-in animation
    const menuContainer = document.querySelector('.relative.z-10');
    menuContainer.innerHTML = `
      <div class="text-xl md:text-4xl lg:text-6xl font-extrabold text-white mb-8 lg:mb-16 tracking-tighter drop-shadow-lg">
        Renzora Season ${this.currentSeason}
      </div>
      <div id="story-mode-tab" class="menu-item text-4xl md:text-5xl lg:text-6xl font-extrabold text-white mb-6 md:mb-8 lg:mb-10 cursor-pointer hover:text-yellow-500 tracking-tighter drop-shadow-lg">
        Story Mode
      </div>
      <div id="online-tab" class="menu-item text-4xl md:text-5xl lg:text-6xl font-extrabold text-white mb-6 md:mb-8 lg:mb-10 cursor-pointer hover:text-yellow-500 tracking-tighter drop-shadow-lg">
        Online
      </div>
      <div id="community-tab" class="menu-item text-4xl md:text-5xl lg:text-6xl font-extrabold text-white mb-6 md:mb-8 lg:mb-10 cursor-pointer hover:text-yellow-500 tracking-tighter drop-shadow-lg">
        Store
      </div>
      <div id="settings-tab" class="menu-item text-4xl md:text-5xl lg:text-6xl font-extrabold text-white cursor-pointer hover:text-yellow-500 tracking-tighter drop-shadow-lg">
        Settings
      </div>
    `;

    this.flyInMenuItems(); // Trigger the fly-in effect for the main menu
    this.currentMenuIndex = 0; // Set the first item as active
    this.highlightMenuItem(this.currentMenuIndex); // Highlight the first item
  },

  highlightMenuItem: function (index) {
    const menuItems = document.querySelectorAll('.menu-item');
    menuItems.forEach((item, i) => {
      const isSelected = i === index;
      item.classList.toggle('text-yellow-500', isSelected); // Highlight selected menu item
      item.classList.toggle('text-white', !isSelected); // Unhighlight others
    });
  },

  navigateMenu: function (direction) {
    const menuItems = document.querySelectorAll('.menu-item');
    this.currentMenuIndex = (this.currentMenuIndex + direction + menuItems.length) % menuItems.length;
    this.highlightMenuItem(this.currentMenuIndex);
    audio.playAudio("click", assets.load('click'), 'sfx');
  }
};

main_title_window.start();

</script>

</div>
