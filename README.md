<h1 align="center">A game engine that renders to javascript canvas for web</h1>

> ⚠️ **Warning**  
> This is a very early development version and is extremely unstable and will most likely break or cause problems. Only use if you intend to help with development.

![](https://i.imgur.com/V2j1yIL.png)

# Requirements
- Docker Desktop: https://www.docker.com/products/docker-desktop
- MongoDB Software: https://www.mongodb.com/products/tools/compass

# Installing Renzora Engine

Open your terminal of choice (make sure docker desktop is running)
```
git clone https://github.com/renzora/engine.git
cd engine
```
Before starting the server make sure you edit ```.env``` to change the environment variables to suit your needs.

```
docker-compose up --build
```

# Services
- Client: Nginx ```(port 80/443)```
- Website: ```http://localhost```
- Nodejs Express endpoint: ```http://localhost:3000```

# Default Login
- Username: ```admin```
- Password: ```password```

# Default MongoDB
- URI: ```mongodb://admin:this_is_a_test_password@localhost:27017/```
- Host: ```localhost```
- Port: ```27017```
- Database Name: ```renzora```
- Mongo Username: ```admin```
- Mongo Password: ```this_is_a_test_password```

# Webpack (minifies tailwind css/js)
```
cd server
npx webpack --watch
```

# Notes
"WebSocket connection to 'wss://localhost:3000/' failed"
- because the local dev server is using a self-signed ssl certificate and not a domain specific CA certificate; browsers by default don't trust it. to get around this issue, visit ```https://localhost:3000``` You will be presented with a screen saying connection is not private. For the purposes of development you can click on Advanced and then proceed anyway. clicking the proceed link wont actually do anything. You can close the window immediately. Then refresh the website at ```http://localhost``` and the websocket should now connect. You shouldn't have this issue on a production server using full ssl.
