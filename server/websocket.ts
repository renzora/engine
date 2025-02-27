// websocket.ts
import { handleChatMessage } from './live/chat';

interface WSClient {
  ws: WebSocket;
  state?: Record<string, any>;
}

const wsClients: Map<string, WSClient> = new Map();

export function initializeWebSocket() {
  const wss = Bun.serve({
    port: 3001,
    fetch(req, server) {
      console.log('WS Connection attempt:', req.url);
      
      try {
        if (req.headers.get('upgrade')?.toLowerCase() === 'websocket') {
          const success = server.upgrade(req);
          if (success) {
            console.log('WebSocket upgrade successful');
          } else {
            console.log('WebSocket upgrade failed');
          }
          return success;
        }
      } catch (err) {
        console.error('Error during WebSocket upgrade:', err);
      }
      
      return new Response('Not found', { status: 404 });
    },
    websocket: {
      open(ws) {
        console.log('WebSocket client connected');
      },

      message(ws, rawMessage) {
        try {
          const data = JSON.parse(rawMessage.toString());

          switch (data.command) {
            case 'playerStateUpdate':
              if (data.data?.id) {
                wsClients.set(data.data.id, {
                  ws,
                  state: data.data,
                });

                for (const [_, client] of wsClients) {
                  if (client.ws.readyState === ws.OPEN) {
                    client.ws.send(JSON.stringify({
                      command: 'playerStateUpdate',
                      data: data.data,
                    }));
                  }
                }
              }
              break;

            case 'reloadData':
              for (const [_, client] of wsClients) {
                if (client.ws.readyState === ws.OPEN) {
                  client.ws.send(JSON.stringify({ command: 'reloadData' }));
                }
              }
              break;

            case 'playerDisconnected':
              if (data.data?.id) {
                wsClients.delete(data.data.id);

                for (const [_, client] of wsClients) {
                  if (client.ws.readyState === ws.OPEN) {
                    client.ws.send(JSON.stringify({
                      command: 'playerDisconnected',
                      data: { id: data.data.id },
                    }));
                  }
                }
              }
              break;

            case 'chatMessage':
              handleChatMessage(data, wsClients, ws);
              break;

            default:
              console.warn('Unknown command:', data.command);
          }
        } catch (err) {
          console.error('WebSocket message error:', err);
        }
      },

      close(ws) {
        console.log('WebSocket client disconnected');

        for (const [id, client] of wsClients.entries()) {
          if (client.ws === ws) {
            wsClients.delete(id);

            for (const [_, otherClient] of wsClients) {
              if (otherClient.ws.readyState === ws.OPEN) {
                otherClient.ws.send(JSON.stringify({
                  command: 'playerDisconnected',
                  data: { id },
                }));
              }
            }
            break;
          }
        }
      },
    },
  });

  console.log(`WebSocket server running on port ${wss.port}`);
  return wss;
}