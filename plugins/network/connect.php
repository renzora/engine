<div data-window='network_connect_window' class='window fade-in-scale pixel-corners shadow-lg' style='width: 500px; background: #122f5d;'>

  <div data-part='handle' class='window_title text-yellow-100 p-2 rounded-t' style="background: #122f5d;">
    <div data-part='title' class='title_bg window_border' style="color: #a1b3cf; background: #122f5d;">Renzora Online</div>
  </div>
  
  <div class='clearfix'></div>
  
  <div class='relative'>
    <div class='container text-white p-4 text-center'>
      <!-- Connecting message and spinner -->
      <div class="inline-flex items-center justify-center">
        <p class="text-xl m-0">Connecting to server</p>
        <div class="spinner ml-2"></div>
      </div>
      <!-- Cancel Button -->
      <div class="mt-4">
        <button class="px-6 py-2 bg-blue-400 hover:bg-blue-500 text-blue-900 font-bold rounded-lg transition duration-300" 
                onclick="network_connect_window.bButton()">
          [B] Cancel
        </button>
      </div>
    </div>
  </div>

  <script>
var network_connect_window = {
    keydownHandler: null,

    start: function() {
        document.getElementById('main_title_window_screen').classList.add('hidden');
        this.updateMessage("Connecting to server...");
        network.init();

        // Attach the keydown handler
        this.keydownHandler = (event) => {
            switch (event.key.toLowerCase()) {
                case "b":
                case "escape":
                    this.cancel();
                    break;
                default:
                    break;
            }
        };

        document.addEventListener("keydown", this.keydownHandler);

        // Listen for the 'playerConnected' event
        document.addEventListener('playerConnected', (event) => {
            console.log('Event received:', event.detail); // Log the received detail object
            this.handlePlayerConnected(event.detail); // Handle the event
        });
    },

    handlePlayerConnected: function(data) {
        // Update UI or perform actions based on the event data
        this.updateMessage('Successfully connected to server');
        setTimeout(() => {
            plugin.close('network_connect_window');
            main_title_window.updateMenu('online');
        }, 2000);
    },

    cancel: function() {
        this.updateMessage("Connection canceled.");
        document.getElementById('main_title_window_screen').classList.remove('hidden');
        plugin.close('network_connect_window');
    },

    updateMessage: function(message) {
        const messageElement = document.querySelector('.container p');
        if (messageElement) {
            messageElement.textContent = message;
        }
    },

    unmount: function() {
        if (this.keydownHandler) {
            document.removeEventListener("keydown", this.keydownHandler);
            this.keydownHandler = null;
        }
    }
};


    network_connect_window.start();
  </script>

  <style>
    .spinner {
      width: 20px;
      height: 20px;
      border: 3px solid transparent;
      border-top: 3px solid #a1b3cf; /* Loader color */
      border-radius: 50%;
      animation: spin 1s linear infinite;
    }

    @keyframes spin {
      from {
        transform: rotate(0deg);
      }
      to {
        transform: rotate(360deg);
      }
    }

    .fade-in-scale {
      opacity: 0;
      transform: scale(0.7);
      animation: fadeInScale 0.3s ease-in forwards;
    }

    @keyframes fadeInScale {
      from {
        opacity: 0;
        transform: scale(0.8);
      }
      to {
        opacity: 1;
        transform: scale(1);
      }
    }

    p {
      margin: 0; /* Prevent extra spacing */
    }
  </style>

</div>
