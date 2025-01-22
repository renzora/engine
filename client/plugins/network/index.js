window[id] = {
    id: id,
    ws_uri: "wss://localhost:3000",
    socket: null,

    start: function() {
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
    },

    sendReloadRequest: function() {
        this.send({ command: 'reloadData' });
    }
}