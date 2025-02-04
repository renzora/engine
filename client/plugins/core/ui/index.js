ui = {
    isMobile() {
        const userAgent = navigator.userAgent || navigator.vendor || window.opera;
        if (/android/i.test(userAgent)) {
          return true;
        }
        if (/iPad|iPhone|iPod/.test(userAgent) && !window.MSStream) {
          return true;
        }
        return window.innerWidth <= 768;
      },
    
      fullScreen() {
        const element = document.documentElement;
        if (!document.fullscreenElement && !document.webkitFullscreenElement && !document.msFullscreenElement) {
          if (element.requestFullscreen) {
            element.requestFullscreen().catch((err) => {
              console.error(`Error attempting to enable fullscreen mode: ${err.message}`);
            });
          } else if (element.webkitRequestFullscreen) {
            element.webkitRequestFullscreen();
          } else if (element.msRequestFullscreen) {
            element.msRequestFullscreen();
          }
          if (/android/i.test(navigator.userAgent)) {
            document.addEventListener('fullscreenchange', () => {
              if (document.fullscreenElement) {
                window.scrollTo(0, 1);
              }
            });
          }
        } else {
          if (document.exitFullscreen) {
            document.exitFullscreen().catch((err) => {
              console.error(`Error attempting to exit fullscreen mode: ${err.message}`);
            });
          } else if (document.webkitExitFullscreen) {
            document.webkitExitFullscreen();
          } else if (document.msExitFullscreen) {
            document.msExitFullscreen();
          }
        }
      }
}