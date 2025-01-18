FROM node:alpine
RUN apk add --no-cache \
    python3 \
    make \
    g++ \
    cairo-dev \
    pango-dev \
    jpeg-dev \
    giflib-dev \
  && ln -sf /usr/bin/python3 /usr/bin/python
  
WORKDIR /usr/src/app
COPY server/package*.json ./
RUN --mount=type=cache,target=/root/.npm npm ci
COPY server/ .
CMD ["npm", "run", "dev"]
