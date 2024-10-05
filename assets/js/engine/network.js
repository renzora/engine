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
        if (!this.getToken('renaccount')) {
            
        }
        audio.start();
        game.init();
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
        if (this.gameSocket) {
            this.gameSocket.close();
        }
    },

    close: function(e) {
        modal.closeAll();
        modal.load("errors/blank.php", "error_window", "Server Error", true);
    },

    getToken: function(name) {
        var value = "; " + document.cookie;
        var parts = value.split("; " + name + "=");
        if (parts.length == 2) return parts.pop().split(";").shift();
    },

    getPlayerId: function() {
        // Implement this function to return the player ID
        // For example, this could be stored in a cookie or local storage
        return this.getToken('renaccount');
    },

    sendReloadRequest: function() {
        this.send({ command: 'reloadData' });
    }
};

document.addEventListener('DOMContentLoaded', (e) => {
    network.init();
    input.init(e);
    gamepad.init(e);
});
