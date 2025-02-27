// /server/handlers/chat.ts

interface ChatMessage {
    sender: string;
    senderId: string;
    content: string;
    timestamp: number;
    sceneId: string;
  }
  
  interface ChatRequest {
    playerId: string;
    message: string;
    sceneId: string;
  }
  
  // Validate incoming chat request
  function validateChatRequest(data: any): data is ChatRequest {
    return (
      data &&
      typeof data.playerId === 'string' &&
      typeof data.message === 'string' &&
      typeof data.sceneId === 'string' &&
      data.message.trim().length > 0
    );
  }
  
  // Process a message to prepare it for broadcast
  function processMessage(playerId: string, message: string, sceneId: string, getPlayerInfo: (id: string) => any): ChatMessage {
    return {
      sender: getPlayerInfo(playerId)?.name || 'Unknown Player',
      senderId: playerId,
      content: message,
      timestamp: Date.now(),
      sceneId: sceneId
    };
  }
  
  // Broadcast message to all clients in the same scene
  function broadcastMessage(
    message: ChatMessage, 
    clients: Map<string, any>,
    socketReadyState: number,
    sendFn: (client: any, data: string) => void
  ): void {
    for (const [_, client] of clients) {
      if (client.ws.readyState === socketReadyState && 
          client.state?.sceneId === message.sceneId) {
        sendFn(client.ws, JSON.stringify({
          command: 'chatMessage',
          data: message
        }));
      }
    }
  }
  
  // Log chat messages to console
  function logMessage(message: ChatMessage): void {
    console.log(`Chat: [${message.sceneId}] ${message.sender}: ${message.content}`);
  }
  
  // Main handler function to be called from websocket.ts
  export function handleChatMessage(data: any, wsClients: Map<string, any>, ws: any): void {
    if (!data.data || !validateChatRequest(data.data)) {
      console.warn('Invalid chat message format:', data);
      return;
    }
    
    const { playerId, message, sceneId } = data.data;
    
    const getPlayerInfo = (id: string) => wsClients.get(id)?.state;
    
    const processedMessage = processMessage(playerId, message, sceneId, getPlayerInfo);
    
    broadcastMessage(processedMessage, wsClients, ws.OPEN, (client, data) => client.send(data));
    
    logMessage(processedMessage);
  }