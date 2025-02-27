network = {
  ws_uri: "ws://" + window.location.host + "/ws",
  socket: null,

  start: function() {
    this.connect();
    console.log('plugin network started');
  },

  connect: function() {
    try {
      console.log('Attempting WebSocket connection to:', this.ws_uri);
      this.socket = new WebSocket(this.ws_uri);

      this.socket.onerror = (error) => {
        console.error('WebSocket error:', error);
      };

      this.socket.onopen = (e) => {
        console.log('WebSocket connection established');
        this.open(e);
      };

      this.socket.onmessage = (e) => {
        this.message(e);
      };

      this.socket.onclose = (e) => {
        this.close(e);
      };

      window.onbeforeunload = (e) => {
        this.beforeUnload(e);
      };
    } catch (error) {
      console.error('Failed to create WebSocket:', error);
    }
  },

  open: function(e) {
    console.log("Connected to the WebSocket server.");
  },

  send: function(message) {
    if (this.socket && this.socket.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(message));
      console.log("Sent message:", JSON.stringify(message));
    } else {
      console.error("WebSocket is not open. Message not sent.");
    }
  },

  message: function(e) {
    var json = JSON.parse(e.data);
    console.log("Received message:", json);
    document.dispatchEvent(new CustomEvent(json.command, { detail: json }));
  },

  beforeUnload: function(event) {
    if (this.socket && this.socket.readyState === WebSocket.OPEN) {
      this.send({ command: 'playerDisconnected', data: { id: this.getPlayerId() } });
    }
    if (this.socket) {
      this.socket.close();
    }
  },

  close: function(e) {
    console.error("Disconnected from the server.");
  }
};