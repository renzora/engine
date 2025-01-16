FROM node:alpine
WORKDIR /usr/src/app/server
RUN ls -la
COPY server/package*.json ./
RUN npm install
COPY server .
CMD ["npm", "run", "dev"]