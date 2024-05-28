var network = {
  ws_uri: "wss://localhost:3000",
  socket: null,

  init: function() {
      this.socket = new WebSocket(this.ws_uri);

      this.socket.onopen = (e) => {
          this.open(e);
      };

      this.socket.onmessage = (e) => {
          this.message(e);
      };

      window.onbeforeunload = (e) => {
          this.beforeUnload(e);
      };

      this.socket.onclose = (e) => {
          this.close(e);
      };
  },

  open: function(e) {
      if(!this.getToken('renaccount')) {
          modal.load('auth');
      }

      modal.load('ui');
      console.log("Connected to Main renzora server");
      game.init();
  },

send: function(message) {
    if(this.socket && this.socket.readyState === WebSocket.OPEN) {
        this.socket.send(JSON.stringify(message));
        console.log(JSON.stringify(message));
    } else {
        console.error("WebSocket is not open. Message not sent.");
    }
},

  message: function(e) {
      var json = JSON.parse(e.data);
      document.dispatchEvent(new CustomEvent(json.command, { detail: json }));
  },

  beforeUnload: function(event) {
      if (this.socket) {
          this.socket.close();
      }
      if (this.gameSocket) {
          this.gameSocket.close();
      }
  },

  close: function(e) {
      modal.closeAll();
      modal.load("servers/errors/mainServer.php", "error_window");
  },

  getToken: function(name) {
      var value = "; " + document.cookie;
      var parts = value.split("; " + name + "=");
      if (parts.length == 2) return parts.pop().split(";").shift();
  },
  noCache: function(url) {
    const timestamp = new Date().getTime();
    if (url.includes('?')) {
      return `${url}t=${timestamp}`;
    } else {
      return `${url}?t=${timestamp}`;
    }
  }
};
