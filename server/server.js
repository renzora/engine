const fs = require('fs');
const https = require('https');
const WebSocket = require('ws');

const serverOptions = {
  cert: fs.readFileSync('./certs/cert.pem'),
  key: fs.readFileSync('./certs/key.pem')
};

const server = https.createServer(serverOptions);
const wss = new WebSocket.Server({ server });

console.log("WebSocket Server running on port 3000");

let players = {}; // Store player states

wss.on("connection", ws => {
    console.log("Web Client connected to main websocket server");

    ws.on("message", message => {
        try {
            const data = JSON.parse(message);
            console.log("Received message:", data);

            if (data.command === 'playerStateUpdate') {
                // Ensure the player data has an ID
                if (data.data.id) {
                    players[data.data.id] = data.data;
                    console.log("Updated player state:", players[data.data.id]);

                    // Broadcast the player state update to all connected clients
                    wss.clients.forEach(client => {
                        if (client.readyState === WebSocket.OPEN) {
                            client.send(JSON.stringify({ command: 'playerStateUpdate', data: data.data }));
                        }
                    });
                } else {
                    console.error("Received playerStateUpdate with no ID:", data.data);
                }
            } else if (data.command === 'reloadData') {
                // Broadcast the reload command to all connected clients
                wss.clients.forEach(client => {
                    if (client.readyState === WebSocket.OPEN) {
                        client.send(JSON.stringify({ command: 'reloadData' }));
                    }
                });
            } else if (data.command === 'playerDisconnected') {
                const playerId = data.data.id;
                if (players[playerId]) {
                    delete players[playerId];

                    // Notify all connected clients about the player disconnection
                    wss.clients.forEach(client => {
                        if (client.readyState === WebSocket.OPEN) {
                            client.send(JSON.stringify({ command: 'playerDisconnected', data: { id: playerId } }));
                        }
                    });
                } else {
                    console.error("Received playerDisconnected for unknown player ID:", playerId);
                }
            } if (data.command === 'chatMessage') {
                // Broadcast chat message to all clients
                wss.clients.forEach(client => {
                    if (client.readyState === WebSocket.OPEN) {
                        client.send(JSON.stringify({ command: 'chatMessage', data: data.data }));
                    }
                });
            }
        } catch (e) {
            console.error("Error handling message:", e.message);
        }
    });

    ws.on('close', () => {
        console.log('Web Client disconnected');

        // Find and remove the disconnected player
        for (const id in players) {
            if (players[id].socket === ws) {
                const disconnectedPlayerId = id;
                delete players[id];

                // Notify all connected clients about the player disconnection
                wss.clients.forEach(client => {
                    if (client.readyState === WebSocket.OPEN) {
                        client.send(JSON.stringify({ command: 'playerDisconnected', data: { id: disconnectedPlayerId } }));
                    }
                });
                break;
            }
        }
    });

    // Send the existing players to the newly connected client
    ws.send(JSON.stringify({ command: 'existingPlayers', data: Object.values(players) }));
});

server.listen(3000, () => {
  console.log('Server is listening on port 3000');
});
