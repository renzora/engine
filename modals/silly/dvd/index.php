<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='dvd_window' class='fixed w-full h-full'>

    <div id="dvd_logo" class="bg-yellow-100 text-black p-16 rounded-lg text-6xl font-extrabold">Renzora</div>

    <script>
      var dvd_window = {
        logo: null,
        container: null,
        speedX: 4, // Speed of movement
        speedY: 4,
        posX: 0,
        posY: 0,
        containerWidth: 0,
        containerHeight: 0,
        logoWidth: 0,
        logoHeight: 0,

        start: function() {
          this.logo = document.getElementById('dvd_logo');
          this.container = document.querySelector('[data-window="dvd_window"]');

          // Set initial container size and logo position
          this.handleResize();

          // Initial positioning of the logo
          this.posX = Math.random() * (this.containerWidth - this.logoWidth);
          this.posY = Math.random() * (this.containerHeight - this.logoHeight);

          this.logo.style.position = 'absolute';
          this.updatePosition();
          this.moveLogo();

          // Handle window resize
          window.addEventListener('resize', this.handleResize.bind(this));
        },

        handleResize: function() {
          // Set the container width and height to the dvd_window container's dimensions
          this.containerWidth = this.container.offsetWidth;
          this.containerHeight = this.container.offsetHeight;
          this.logoWidth = this.logo.offsetWidth;
          this.logoHeight = this.logo.offsetHeight;

          // Ensure the logo stays within bounds after resize
          if (this.posX + this.logoWidth > this.containerWidth) {
            this.posX = this.containerWidth - this.logoWidth;
          }
          if (this.posY + this.logoHeight > this.containerHeight) {
            this.posY = this.containerHeight - this.logoHeight;
          }
          this.updatePosition();
        },

        moveLogo: function() {
          var self = this;

          function update() {
            // Move the logo
            self.posX += self.speedX;
            self.posY += self.speedY;

            // Check for collisions with the container edges
            if (self.posX <= 0) {
              self.posX = 0;
              self.speedX *= -1; // Reverse horizontal direction
              self.changeColor();
            }
            if (self.posX + self.logoWidth >= self.containerWidth) {
              self.posX = self.containerWidth - self.logoWidth;
              self.speedX *= -1; // Reverse horizontal direction
              self.changeColor();
            }
            if (self.posY <= 0) {
              self.posY = 0;
              self.speedY *= -1; // Reverse vertical direction
              self.changeColor();
            }
            if (self.posY + self.logoHeight >= self.containerHeight) {
              self.posY = self.containerHeight - self.logoHeight;
              self.speedY *= -1; // Reverse vertical direction
              self.changeColor();
            }

            self.updatePosition();
            requestAnimationFrame(update); // Continue the animation
          }

          requestAnimationFrame(update); // Start the animation
        },

        updatePosition: function() {
          // Update the position of the logo
          this.logo.style.left = this.posX + 'px';
          this.logo.style.top = this.posY + 'px';
        },

        changeColor: function() {
          // Change the logo color to a random one on collision
          var randomColor = '#' + Math.floor(Math.random() * 16777215).toString(16);
          this.logo.style.backgroundColor = randomColor;
        },

        unmount: function() {
          // Clean up code
        }
      }

      // Start the animation immediately
      dvd_window.start();
    </script>
  </div>
<?php
}
?>
