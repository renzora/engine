# Requirements
- Docker Desktop: https://www.docker.com/products/docker-desktop
- Github Desktop: https://desktop.github.com
- MongoDB Software: https://www.mongodb.com/products/tools/compass


# Installing on Windows WSL

Open your terminal of choice (make sure docker desktop is running)
```
git clone https://github.com/renzora/engine.git
cd engine
docker-compose up --build
```

# Notes
"WebSocket connection to 'wss://localhost:3000/' failed"
- because the local dev server is using a self-signed ssl certificate and not a domain specific CA certificate; browsers by default don't trust it. to get around this issue, visit ```https://localhost:3000``` You will be presented with a screen saying connection is not private. For the purposes of renzora development you can click on Advanced and then proceed anyway. clicking the proceed link wont actually do anything. You can close the window immediately. Then refresh the website at ```http://localhost``` and the websocket should now connect.

# Services
- Server: Nginx (gzip compression)
- Website: ```http://localhost```
- Websocket: ```wss://localhost:3000```

# Renzora Login
- Username: ```admin```
- Password: ```password```
- JWT key: ```key```

# MongoDB
- URI: mongodb://localhost:27017/
- Host: ```localhost```
- Port: ```27017```
- Database Name: ```renzora```
- Mongo Username: ```admin```
- Mongo Password: ```password```

# webpack
to compile tailwindcss and watch for new changes, open up a terminal
```
cd server
npx webpack --minify --watch
```
