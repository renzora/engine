<div class='fixed inset-0 bg-black bg-opacity-40 flex items-center justify-center'>
  
<div class="absolute inset-0 z-[-2]">
  <div class="glitch-overlay"></div>
  <div class="color-shift"></div>
  <div class="dark-overlay"></div>

</div>

  <!-- Top Menu and HUD -->
  <div class="fixed top-0 left-0 z-10 w-full flex items-center justify-between px-8 pt-8">
    <!-- Left: L1 Button, Top Menu, and R1 Button -->
    <div class="flex items-center space-x-8">
      <!-- L1 Button -->
      <button 
        class="nav-icon relative flex items-center justify-center l1-button"
        onclick="online_menu_window.l1Button()">
        <span class="trigger-shape"></span>
        <span class="nav-text">L1</span>
      </button>

      <!-- Top Menu -->
      <button 
        data-tab="explore" 
        class="tab-item text-3xl font-extrabold text-white hover:text-yellow-400" 
        onclick="online_menu_window.showTab(0)">
        World Explore
      </button>
      <button 
        data-tab="arena" 
        class="tab-item text-3xl font-extrabold text-white hover:text-yellow-400" 
        onclick="online_menu_window.showTab(1)">
        Battle Arena
      </button>
      <button 
        data-tab="training" 
        class="tab-item text-3xl font-extrabold text-white hover:text-yellow-400" 
        onclick="online_menu_window.showTab(2)">
        Training
      </button>
      <button 
        data-tab="store" 
        class="tab-item text-3xl font-extrabold text-white hover:text-yellow-400" 
        onclick="online_menu_window.showTab(3)">
        Store
      </button>
      <button 
        data-tab="exit" 
        class="tab-item text-3xl font-extrabold text-white hover:text-yellow-400" 
        onclick="online_menu_window.showTab(4)">
        Exit
      </button>

      <!-- R1 Button -->
      <button 
        class="nav-icon relative flex items-center justify-center r1-button"
        onclick="online_menu_window.r1Button()">
        <span class="trigger-shape"></span>
        <span class="nav-text">R1</span>
      </button>
    </div>

    <!-- Right: Player HUD and R1 Button -->
    <div class="flex items-center space-x-8">
      <!-- Player HUD -->
      <div class="text-white text-right">
        <p class="text-lg font-bold"><span class="text-yellow-400">Player123</span></p>
        <p class="text-sm">Level: <span class="text-yellow-400">15</span> | XP: <span class="text-yellow-400">450/500</span></p>
        <p class="text-sm">W: <span class="text-green-400">12</span> | D: <span class="text-gray-400">3</span> | L: <span class="text-red-400">5</span></p>
      </div>

    </div>
  </div>




  <div id="online_menu_window_screen" class="tab-content-container flex flex-col items-center justify-center">

  <div id="online_menu_explore" class="tab-screen hidden w-full max-w-3xl space-y-4 px-4" data-parent="renzora">
      <div class="card bg-black bg-opacity-60 text-white rounded-lg shadow-lg p-8" data-callback="characterSelection">
        <h2 class="text-3xl font-bold text-yellow-400">New Story</h2>
        <p class="text-gray-300 mt-4">Start a fresh adventure in Renzora.</p>
        <button class="select-btn hidden absolute bottom-4 right-4 text-yellow-400 text-xl">[A] Select</button>
      </div>
      <div class="card bg-black bg-opacity-60 text-white rounded-lg shadow-lg p-8" data-callback="loadSavedGame">
        <h2 class="text-3xl font-bold text-yellow-400">Load Story</h2>
        <p class="text-gray-300 mt-4">Continue where you left off.</p>
        <button class="select-btn hidden absolute bottom-4 right-4 text-yellow-400 text-xl">[A] Select</button>
      </div>
    </div>


    <div id="online_menu_arena" class="tab-screen hidden w-full max-w-3xl space-y-4 px-4">
      <div class="card bg-black bg-opacity-60 text-white rounded-lg shadow-lg p-8" data-callback="new_story">
        <h2 class="text-3xl font-bold text-yellow-400">Battle Royale</h2>
        <p class="text-gray-300 mt-4">100 players, 1 survivor</p>
        <button class="select-btn hidden absolute bottom-4 right-4 text-yellow-400 text-xl">[A] Select</button>
      </div>
      <div class="card bg-black bg-opacity-60 text-white rounded-lg shadow-lg p-8" data-callback="network_connect">
        <h2 class="text-3xl font-bold text-yellow-400">Team Battle</h2>
        <p class="text-gray-300 mt-4">Team up to defeat your opponents</p>
        <button class="select-btn hidden absolute bottom-4 right-4 text-yellow-400 text-xl">[A] Select</button>
      </div>
      <div class="card bg-black bg-opacity-60 text-white rounded-lg shadow-lg p-8" data-callback="network_connect">
        <h2 class="text-3xl font-bold text-yellow-400">Heads up</h2>
        <p class="text-gray-300 mt-4">You vs a random opponent</p>
        <button class="select-btn hidden absolute bottom-4 right-4 text-yellow-400 text-xl">[A] Select</button>
      </div>
      <div class="card bg-black bg-opacity-60 text-white rounded-lg shadow-lg p-8" data-callback="network_connect">
        <h2 class="text-3xl font-bold text-yellow-400">Co-op</h2>
        <p class="text-gray-300 mt-4">Team up with a friend to battle</p>
        <button class="select-btn hidden absolute bottom-4 right-4 text-yellow-400 text-xl">[A] Select</button>
      </div>
    </div>

    <div id="online_menu_training" class="tab-screen hidden w-full max-w-3xl space-y-4 px-4" data-parent="story_mode">
      <div class="card bg-black bg-opacity-60 text-white rounded-lg shadow-lg p-8" data-callback="startNewStory">
        <h2 class="text-3xl font-bold text-yellow-400">Select Character</h2>
        <p class="text-gray-300 mt-4">etc etc</p>
        <button class="select-btn hidden absolute bottom-4 right-4 text-yellow-400 text-xl">[A] Select</button>
      </div>
    </div>


    <div id="online_menu_store" class="tab-screen hidden w-full max-w-3xl space-y-4 px-4">
      <div class="card bg-black bg-opacity-60 text-white rounded-lg shadow-lg p-8">
        <h2 class="text-3xl font-bold text-yellow-400">Forums</h2>
        <p class="text-gray-300 mt-4">Join discussions and share your ideas.</p>
        <button class="select-btn hidden absolute bottom-4 right-4 text-yellow-400 text-xl">[A] Select</button>
      </div>
      <div class="card bg-black bg-opacity-60 text-white rounded-lg shadow-lg p-8">
        <h2 class="text-3xl font-bold text-yellow-400">Social Media</h2>
        <p class="text-gray-300 mt-4">Follow us for freebies and latest news/content</p>
        <button class="select-btn hidden absolute bottom-4 right-4 text-yellow-400 text-xl">[A] Select</button>
      </div>
      <div class="card bg-black bg-opacity-60 text-white rounded-lg shadow-lg p-8">
        <h2 class="text-3xl font-bold text-yellow-400">Announcements/Updates</h2>
        <p class="text-gray-300 mt-4">Get all the latest news and updates right here</p>
        <button class="select-btn hidden absolute bottom-4 right-4 text-yellow-400 text-xl">[A] Select</button>
      </div>
    </div>

    <div id="online_menu_exit" class="tab-screen hidden w-full max-w-3xl space-y-4 px-4">
      <div class="card bg-black bg-opacity-60 text-white rounded-lg shadow-lg p-8" data-callback="exit">
        <h2 class="text-3xl font-bold text-yellow-400">Exit</h2>
        <p class="text-gray-300 mt-4">Back to the main menu</p>
        <button class="select-btn hidden absolute bottom-4 right-4 text-yellow-400 text-xl">[A] Select</button>
      </div>
    </div>

  </div>
  </div>

<style>
.dark-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  pointer-events: none; /* Ensure it doesn't interfere with interactions */
  z-index: -1; /* Place behind other elements but above the background */
  background: radial-gradient(
    circle,
    rgba(0, 0, 0, 0) 40%,  /* Start transition closer to the center */
    rgba(0, 0, 0, 0.6) 80%, /* Make edges significantly darker */
    rgba(0, 0, 0, 1) 100%   /* Fully opaque at the very edge */
  );
}

.glitch-overlay,
.color-shift {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: -2; /* Ensure it is behind everything */
  pointer-events: none; /* Prevent it from interfering with interactions */
}

.glitch-overlay {
  background: repeating-linear-gradient(
    45deg,
    rgba(255, 255, 255, 0.05) 0%,
    rgba(255, 255, 255, 0.05) 4%,
    transparent 4%,
    transparent 8%
  );
  opacity: 0.15;
  animation: glitchMove 15s linear infinite;
}

.color-shift {
  background: linear-gradient(135deg, #fff700, #ffd300); /* Bright yellow gradient */
  opacity: 0.1;
  animation: colorShift 30s linear infinite;
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

@keyframes colorShift {
  0% {
    filter: hue-rotate(0deg);
  }
  50% {
    filter: hue-rotate(180deg);
  }
  100% {
    filter: hue-rotate(360deg);
  }
}
.tab-content-container {
  height: 100%;
  width: 100%;
}

.tab-screen {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.card {
  width: 100%;
  max-width: 600px;
  margin: 0 auto;
  position: relative;
  cursor: pointer;
  transform: scale(1); /* Ensure normal cards have no scaling */
  transition: transform 0.3s ease-in-out; /* Add smooth transition */
}

.scale-card {
  transform: scale(1.1); /* Slightly enlarge the card */
  transition: transform 0.3s ease-in-out; /* Smooth scaling animation */
}

.card.flash-opacity::before {
  content: '';
  position: absolute;
  top: -4px; /* Extend beyond the card to create the outer border */
  left: -4px;
  width: calc(100% + 8px); /* Expand the width to account for the border */
  height: calc(100% + 8px); /* Expand the height to account for the border */
  border: 4px solid rgba(255, 223, 0, 1); /* Yellow outer border */
  border-radius: inherit; /* Match the card's rounded corners */
  opacity: 1;
  z-index: -1; /* Ensure it’s behind the card content */
  animation: flash-opacity 1.5s infinite ease-in-out;
}

@keyframes flash-opacity {
  0% {
    opacity: 1; /* Fully visible outer border */
  }
  50% {
    opacity: 0.3; /* Partially faded outer border */
  }
  100% {
    opacity: 1; /* Fully visible outer border */
  }
}

.tab-screen {
  animation: fadeInUp 0.5s ease-out;
}


@keyframes fadeInUp {
  from {
    opacity: 0;
    transform: translateY(100px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.nav-icon {
  width: 80px;
  height: 50px;
  position: relative;
  cursor: pointer;
  text-align: center;
  transition: transform 0.2s;
}

@keyframes buttonZoom {
  0% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.1);
  }
  100% {
    transform: scale(1);
  }
}

.nav-icon.animate {
  animation: buttonZoom 0.2s ease-in-out;
}


.nav-icon:hover {
  transform: scale(1.1); /* Enlarge slightly on hover */
}

.trigger-shape {
  position: absolute;
  width: 100%;
  height: 100%;
  background: linear-gradient(135deg, #333, #444); /* Gradient background for the shape */
  clip-path: polygon(0% 0%, 100% 0%, 85% 100%, 15% 100%);
  box-shadow: 0 4px 8px rgba(0, 0, 0, 0.3); /* Adds shadow for depth */
}

.nav-text {
  position: relative;
  font-size: 1.5rem;
  font-weight: bold;
  color: white;
}
.tab-item {
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.8); /* Adds a dark shadow below the text */
}

.tab-item:hover {
  text-shadow: 0 2px 6px rgba(0, 0, 0, 0.9); /* Intensifies shadow on hover */
}

.select-btn {
  background: none;
  border: none;
  padding: 0;
  font-size: 1.5rem;
  font-weight: bold;
  color: white;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.8);
  pointer-events: none; /* Prevents accidental clicks */
}

.card:hover .select-btn {
  text-shadow: 0 2px 6px rgba(0, 0, 0, 0.9);
}

.select-btn.hidden {
  display: none;
}


</style>

<script>
window[id] = {
  id: id,
  currentTabIndex: 0,
  tabName: "explore",
  tabs: ["explore", "arena", "training", "store", "exit"],
  currentCardIndex: 0,
  throttleDuration: 200, // Throttle duration in milliseconds
  lastButtonPress: 0,
  keydownListener: null, // Placeholder for the keydown listener
  mousemoveListener: null, // Placeholder for the mousemove listener
  cursorTimeout: null, // Timeout for hiding the cursor

  // Initialize the main title window
  start: function () {
    assets.preload([
            { name: 'menu_tab_switch', path: 'audio/sfx/ui/menu_tab_switch.mp3' },
            { name: 'menu_navigate', path: 'audio/sfx/ui/menu_navigate.mp3' },
            { name: 'menu_selection_confirm', path: 'audio/sfx/ui/menu_selection_confirm.mp3' },
        ], () => {
          console.log("menu_tab_switch_loaded");
        });
  camera.panTo(null, null, 0.2, true);
  utils.gameTime.hours = 0;
  game.timeActive = false;
  weather.snowActive = true;
  this.showTab(0);
  this.highlightCard();
  this.highlightCardOnHover(); // Add hover functionality
  this.initializeCardClickEvents(); // Initialize click events

  this.keydownListener = (event) => {
      switch (event.key) {
        case "ArrowUp":
          this.upButton();
          break;
        case "ArrowDown":
          this.downButton();
          break;
        case "ArrowLeft":
          this.leftButton();
          break;
        case "ArrowRight":
          this.rightButton();
          break;
        case "q": // Map "Q" to L1 button
          this.l1Button();
          break;
        case "e": // Map "E" to R1 button
          this.r1Button();
          break;
        case "a": // Map "A" to the select button
        case "Enter": // Map "Enter" to the select button
          this.aButton();
          break;
        case " ": // Spacebar toggles fullscreen
          utils.fullScreen();
          break;
        default:
          break;
      }
    };

  let timeout;

  this.mousemoveListener = () => {
      document.body.style.cursor = "default"; // Show cursor
      clearTimeout(this.cursorTimeout);
      this.cursorTimeout = setTimeout(() => {
        document.body.style.cursor = "none"; // Hide cursor after 3 seconds
      }, 3000);
    };

        // Add the event listeners to the object
        this.addEventListeners();

},

unmount: function () {
    if (this.keydownListener) {
      document.removeEventListener("keydown", this.keydownListener);
      this.keydownListener = null;
    }
    if (this.mousemoveListener) {
      document.removeEventListener("mousemove", this.mousemoveListener);
      this.mousemoveListener = null;
    }
    if (this.cursorTimeout) {
      clearTimeout(this.cursorTimeout);
      this.cursorTimeout = null;
    }
    console.log("Event listeners removed from main_title_window.");
    plugin.load({ id: 'main_title_window', url: 'menus/main_title/index.php', name: 'Main Tiles', drag: true,reload: true });
  },

addEventListeners: function () {
    document.addEventListener("keydown", this.keydownListener);
    document.addEventListener("mousemove", this.mousemoveListener);
    console.log("Event listeners added to online_menu_window.");
  },

initializeCardClickEvents: function () {
    // Add click event listeners to all cards
    document.querySelectorAll(".card").forEach((card) => {
      card.addEventListener("click", () => {
        // Highlight the clicked card
        const activeTab = document.querySelectorAll(".tab-screen")[this.currentTabIndex];
        const cards = Array.from(activeTab.querySelectorAll(".card"));
        const cardIndex = cards.indexOf(card);

        if (cardIndex !== -1) {
          this.currentCardIndex = cardIndex;
          this.highlightCard();

          // Execute the callback function
          const callback = card.getAttribute("data-callback");
          if (callback && typeof this[callback] === "function") {
            this[callback](); // Execute the callback function
          } else {
            console.warn(`Callback function '${callback}' is not defined in online_menu_window.`);
          }
        }
      });
    });
  },

  // Throttle function to limit repeated button presses
  throttle: function (callback) {
    const currentTime = Date.now();
    if (currentTime - this.lastButtonPress < this.throttleDuration) {
      return false;
    }
    this.lastButtonPress = currentTime;
    callback();
    return true;
  },

  // Display the specified tab by index

  showTab: function (tabIdentifier) {
  let index = -1; // Default to -1 for named screens not in `tabs`

  // Determine index if the identifier is a number or exists in `tabs`
  if (typeof tabIdentifier === "number") {
    index = tabIdentifier;
  } else if (typeof tabIdentifier === "string") {
    index = this.tabs.indexOf(tabIdentifier);
  }

  // If index is invalid, fallback to using the name directly
  const isNamedScreen = index === -1;
  const newTabName = isNamedScreen ? tabIdentifier : this.tabs[index]; // Determine the new active tab name

  const newScreenId = `online_menu_${newTabName}`;

  // Handle invalid screen names gracefully
  const newScreen = document.getElementById(newScreenId);
  if (!newScreen) {
    console.error(`Screen "${newScreenId}" not found in the DOM.`);
    return;
  }

  // Hide all screens
  document.querySelectorAll(".tab-screen").forEach((screen) => {
    screen.style.display = "none";
    screen.classList.remove("fade-in-up"); // Remove animation class if applied
  });

  // Update the current tab name
  this.tabName = newTabName;

  // Update the current index if using the `tabs` array
  if (!isNamedScreen) {
    this.currentTabIndex = index;
  }

  // Show the new screen
  newScreen.style.display = "block";
  newScreen.classList.add("fade-in-up"); // Add animation to the active screen

  // Update active tab styles only for indexed tabs
  if (!isNamedScreen) {
    document.querySelectorAll(".tab-item").forEach((item, idx) => {
      item.classList.toggle("active", idx === index);
      item.classList.toggle("text-yellow-400", idx === index);
    });
  }

  // Highlight the first card in the new screen
  this.resetCardSelection();
},


  // Reset card selection to the first card
  resetCardSelection: function () {
    this.currentCardIndex = 0; // Always reset to the first card
    this.highlightCard();
  },

  // Highlight the currently selected card
  highlightCard: function () {
  const activeTab = document.getElementById(this.tabName); // Use tabName to find the active screen
  if (!activeTab) {
    console.warn(`No active screen found for "${this.tabName}".`);
    return;
  }

  const cards = activeTab.querySelectorAll(".card");
  
  // Log the number of cards found
  console.log(`Screen "${this.tabName}" has ${cards.length} card(s).`);
  
  if (cards.length === 0) {
    console.warn(`No cards found in the current screen "${this.tabName}".`);
    return;
  }

  cards.forEach((card, idx) => {
    const selectBtn = card.querySelector(".select-btn");
    if (idx === this.currentCardIndex) {
      card.classList.add("flash-opacity");
      card.classList.add("scale-card");
      if (selectBtn) selectBtn.classList.remove("hidden");
    } else {
      card.classList.remove("flash-opacity");
      card.classList.remove("scale-card");
      if (selectBtn) selectBtn.classList.add("hidden");
    }
  });
},


  highlightCardOnHover: function () {
  // Add event listeners to each card
  document.querySelectorAll(".tab-screen").forEach((tabScreen, tabIndex) => {
    const cards = tabScreen.querySelectorAll(".card");
    cards.forEach((card, cardIndex) => {
      card.addEventListener("mouseenter", () => {
        if (this.currentTabIndex === tabIndex) {
          this.currentCardIndex = cardIndex;
          this.highlightCard();
        }
      });
    });
  });
},

  // Navigate to the previous tab
  l1Button: function () {
    this.throttle(() => {
      const previousIndex = (this.currentTabIndex - 1 + this.tabs.length) % this.tabs.length;
      this.showTab(previousIndex);
      this.animateButton(".l1-button");
      audio.playAudio("menu_tab_switch", assets.use('menu_tab_switch'), 'sfx');
    });
  },

  // R1 button navigates to the next tab
  r1Button: function () {
    this.throttle(() => {
      const nextIndex = (this.currentTabIndex + 1) % this.tabs.length;
      this.showTab(nextIndex);
      this.animateButton(".r1-button");
      audio.playAudio("menu_tab_switch", assets.use('menu_tab_switch'), 'sfx');
    });
  },

  // Navigate to the previous card within the current tab
  upButton: function () {
  this.throttle(() => {
    const activeTab = document.getElementById(this.tabName); // Use tabName directly
    if (!activeTab) {
      console.warn(`No active screen found for "${this.tabName}".`);
      return;
    }

    const cards = Array.from(activeTab.querySelectorAll(".card"));

    if (cards.length === 0) {
      console.log(`No cards found in screen "${this.tabName}".`);
      return;
    }

    // Move up in the card list
    this.currentCardIndex = (this.currentCardIndex - 1 + cards.length) % cards.length;
    console.log(`Navigating to card index ${this.currentCardIndex} in screen "${this.tabName}".`);
    this.highlightCard();

    // Play navigation sound effect
    audio.playAudio("menu_navigate", assets.use('menu_navigate'), 'sfx');
  });
},

downButton: function () {
  this.throttle(() => {
    const activeTab = document.getElementById(this.tabName); // Use tabName directly
    if (!activeTab) {
      console.warn(`No active screen found for "${this.tabName}".`);
      return;
    }

    const cards = Array.from(activeTab.querySelectorAll(".card"));

    if (cards.length === 0) {
      console.log(`No cards found in screen "${this.tabName}".`);
      return;
    }

    // Move down in the card list
    this.currentCardIndex = (this.currentCardIndex + 1) % cards.length;
    console.log(`Navigating to card index ${this.currentCardIndex} in screen "${this.tabName}".`);
    this.highlightCard();

    // Play navigation sound effect
    audio.playAudio("menu_navigate", assets.use('menu_navigate'), 'sfx');
  });
},


  leftButton: function () {
  this.throttle(() => {
    this.currentTabIndex = (this.currentTabIndex - 1 + this.tabs.length) % this.tabs.length;
    this.showTab(this.currentTabIndex);

    // Trigger animation for L1 button
    const l1Button = document.querySelector(".l1-button");
    if (l1Button) {
      audio.playAudio("menu_tab_switch", assets.use('menu_tab_switch'), 'sfx');
      l1Button.classList.add("animate");
      setTimeout(() => l1Button.classList.remove("animate"), 200);
    }
  });
},

rightButton: function () {
  this.throttle(() => {
    this.currentTabIndex = (this.currentTabIndex + 1) % this.tabs.length;
    this.showTab(this.currentTabIndex);

    // Trigger animation for R1 button
    const r1Button = document.querySelector(".r1-button");
    if (r1Button) {
      audio.playAudio("menu_tab_switch", assets.use('menu_tab_switch'), 'sfx');
      r1Button.classList.add("animate");
      setTimeout(() => r1Button.classList.remove("animate"), 200);
    }
  });
},

aButton: function () {
  this.throttle(() => {
    const activeTab = document.getElementById(this.tabName); // Use tabName to find the active screen
    if (!activeTab) {
      console.warn(`No active screen found for "${this.tabName}".`);
      return;
    }

    const cards = Array.from(activeTab.querySelectorAll(".card"));
    if (cards.length === 0) return;

    const activeCard = cards[this.currentCardIndex];
    if (!activeCard) return;

    // Retrieve the callback function from the data-callback attribute
    const callback = activeCard.getAttribute("data-callback");

    if (callback && typeof this[callback] === "function") {
      audio.playAudio("menu_selection_confirm", assets.use("menu_selection_confirm"), "sfx");
      this[callback](); // Execute the callback function
    } else {
      console.warn(`Callback function '${callback}' is not defined in online_menu_window.`);
    }
  });
},


  bButton: function () {
    // Get the currently active screen
    this.throttle(() => {
    const activeScreen = document.getElementById(this.tabName);
    if (!activeScreen) {
      console.warn(`No active screen found for "${this.tabName}".`);
      return;
    }

    // Check for the data-parent attribute
    const parentScreen = activeScreen.getAttribute("data-parent");
    if (parentScreen) {
      console.log(`Navigating to parent screen: ${parentScreen}`);
      audio.playAudio("menu_selection_confirm", assets.use('menu_selection_confirm'), 'sfx');
      this.showTab(parentScreen); // Navigate to the parent screen
    } else {
      console.warn(`No parent screen found for "${this.tabName}".`);
    }
  });
  },

  animateButton: function (buttonSelector) {
    const button = document.querySelector(buttonSelector);
    if (button) {
      button.classList.add("animate");
      setTimeout(() => button.classList.remove("animate"), 200);
    }
  },

  exit: function() {
    network.close();
    console.log("closing menu");
    plugin.close('online_menu_window');
  },

  // Sign-in button functionality placeholder
  signIn: function () {
    plugin.load({ id: 'auth_window', url: 'auth/index.php', name: 'SignIn', drag: true,reload: true }); 
  }
};
</script>