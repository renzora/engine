const WebSocket = require('ws');

let wsClients = new Map();

function initializeWebSocket(server) {
    const wss = new WebSocket.Server({ server, path: '/ws' });

    wss.on('connection', ws => {
        console.log('WebSocket client connected');
        
        ws.on('message', async (message) => {
            try {
                const data = JSON.parse(message);
                
                switch (data.command) {
                    case 'playerStateUpdate':
                        if (data.data.id) {
                            wsClients.set(data.data.id, {
                                ws,
                                state: data.data
                            });
                            
                            // Broadcast to all clients
                            wss.clients.forEach(client => {
                                if (client.readyState === WebSocket.OPEN) {
                                    client.send(JSON.stringify({
                                        command: 'playerStateUpdate',
                                        data: data.data
                                    }));
                                }
                            });
                        }
                        break;

                    case 'reloadData':
                        wss.clients.forEach(client => {
                            if (client.readyState === WebSocket.OPEN) {
                                client.send(JSON.stringify({ command: 'reloadData' }));
                            }
                        });
                        break;

                    case 'playerDisconnected':
                        if (data.data.id) {
                            wsClients.delete(data.data.id);
                            wss.clients.forEach(client => {
                                if (client.readyState === WebSocket.OPEN) {
                                    client.send(JSON.stringify({
                                        command: 'playerDisconnected',
                                        data: { id: data.data.id }
                                    }));
                                }
                            });
                        }
                        break;

                    case 'chatMessage':
                        wss.clients.forEach(client => {
                            if (client.readyState === WebSocket.OPEN) {
                                client.send(JSON.stringify({
                                    command: 'chatMessage',
                                    data: data.data
                                }));
                            }
                        });
                        break;
                }
            } catch (err) {
                console.error('WebSocket message error:', err);
            }
        });

        ws.on('close', () => {
            console.log('WebSocket client disconnected');
            // Remove disconnected client
            for (const [id, client] of wsClients.entries()) {
                if (client.ws === ws) {
                    wsClients.delete(id);
                    // Notify other clients
                    wss.clients.forEach(client => {
                        if (client.readyState === WebSocket.OPEN) {
                            client.send(JSON.stringify({
                                command: 'playerDisconnected',
                                data: { id }
                            }));
                        }
                    });
                    break;
                }
            }
        });
    });

    return wss;
}

module.exports = { initializeWebSocket };