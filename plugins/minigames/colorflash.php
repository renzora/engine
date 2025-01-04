<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='colorflash_window' class='window arcade-machine' style='width: 350px;'>

    <div data-part='handle' class='arcade-title'>
      <div class='float-right'>
        <button class="arcade-close" aria-label="Close (ESC)" data-close>&times;</button>
      </div>
      <div data-part='title' class='arcade-header'>Arcade Machine</div>
    </div>
    
    <div class='clearfix'></div>
    
    <div class='relative'>
      <div class='arcade-screen'>
        <p id="gameStatus">Welcome to Color Match!</p>
        <div id="colorButtons" class="grid grid-cols-2 gap-2 mt-4">
          <button class="arcade-button arcade-red" data-color="red"></button>
          <button class="arcade-button arcade-blue" data-color="blue"></button>
          <button class="arcade-button arcade-green" data-color="green"></button>
          <button class="arcade-button arcade-yellow" data-color="yellow"></button>
        </div>
        <button id="startGame" class="start-game-button mt-4">Start Game</button>
      </div>
    </div>

    <script>
      var colorflash_window = {
        gameSequence: [],
        playerSequence: [],
        level: 1,
        colors: ['red', 'blue', 'green', 'yellow'],

        start: function() {
          document.getElementById('startGame').addEventListener('click', this.startGame.bind(this));
          document.querySelectorAll('#colorButtons button').forEach(button => {
            button.addEventListener('click', this.handleColorClick.bind(this));
          });
        },

        startGame: function() {
          this.level = 1;
          this.gameSequence = [];
          this.playerSequence = [];
          this.updateStatus('Memorize the sequence!');
          this.nextLevel();
        },

        nextLevel: function() {
          this.playerSequence = [];
          this.gameSequence.push(this.getRandomColor());
          this.displaySequence();
        },

        displaySequence: function() {
          let i = 0;
          const interval = setInterval(() => {
            this.flashColor(this.gameSequence[i]);
            i++;
            if (i >= this.gameSequence.length) {
              clearInterval(interval);
              setTimeout(() => this.updateStatus('Now repeat the sequence!'), 500);
            }
          }, 800);
        },

        handleColorClick: function(event) {
          const color = event.target.getAttribute('data-color');
          this.playerSequence.push(color);
          this.flashColor(color);

          if (this.playerSequence.length === this.gameSequence.length) {
            if (this.checkSequence()) {
              this.updateStatus('Correct! Next level.');
              setTimeout(() => this.nextLevel(), 1000);
            } else {
              this.updateStatus('Game Over! Try again.');
            }
          }
        },

        checkSequence: function() {
          for (let i = 0; i < this.gameSequence.length; i++) {
            if (this.gameSequence[i] !== this.playerSequence[i]) {
              return false;
            }
          }
          return true;
        },

        getRandomColor: function() {
          return this.colors[Math.floor(Math.random() * this.colors.length)];
        },

        flashColor: function(color) {
          const button = document.querySelector(`button[data-color='${color}']`);
          button.classList.add('flash');
          setTimeout(() => button.classList.remove('flash'), 300);
        },

        updateStatus: function(message) {
          document.getElementById('gameStatus').textContent = message;
        },

        unmount: function() {
          // Clean up code
        }
      }

      colorflash_window.start();
    </script>

    <style>
      /* General Arcade Machine Styling */
      .arcade-machine {
        background: #333;
        border: 5px solid #666;
        border-radius: 15px;
        box-shadow: 0 0 20px #000;
        font-family: 'Press Start 2P', cursive;
      }

      /* Arcade Title */
      .arcade-title {
        background: linear-gradient(45deg, #ff0000, #ff9900);
        text-align: center;
        padding: 15px;
        border-top-left-radius: 10px;
        border-top-right-radius: 10px;
        color: #fff;
        font-size: 16px;
        font-family: 'Press Start 2P', cursive;
      }

      /* Arcade Header */
      .arcade-header {
        font-size: 18px;
        letter-spacing: 2px;
      }

      /* Arcade Close Button */
      .arcade-close {
        background-color: #ff0000;
        color: #fff;
        border: none;
        font-size: 20px;
        cursor: pointer;
      }

      /* Arcade Screen */
      .arcade-screen {
        background-color: #222;
        padding: 10px;
        border: 3px solid #444;
        margin: 10px 0;
        border-radius: 8px;
        height: 200px;
        color: lime;
        text-align: center;
      }

      /* Arcade Buttons */
      .arcade-button {
        height: 60px;
        width: 100%;
        border: none;
        cursor: pointer;
        border-radius: 8px;
        margin: 5px;
      }

      .arcade-red {
        background-color: #ff4d4d;
      }

      .arcade-blue {
        background-color: #4d79ff;
      }

      .arcade-green {
        background-color: #4dff4d;
      }

      .arcade-yellow {
        background-color: #ffff4d;
      }

      .arcade-button.flash {
        opacity: 0.5;
        transition: opacity 0.3s;
      }

      /* Start Button */
      .start-game-button {
        background: linear-gradient(45deg, #ff0000, #ffcc00);
        border: 2px solid #ff9900;
        padding: 10px;
        color: #fff;
        font-size: 12px;
        letter-spacing: 2px;
        cursor: pointer;
        width: 100%;
        text-transform: uppercase;
        border-radius: 8px;
        transition: background 0.2s;
      }

      .start-game-button:hover {
        background: linear-gradient(45deg, #ff9900, #ff0000);
      }

      /* Arcade Font */
      @import url('https://fonts.googleapis.com/css2?family=Press+Start+2P&display=swap');
    </style>

  </div>
<?php
}
?>
