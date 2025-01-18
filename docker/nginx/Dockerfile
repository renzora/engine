FROM nginx:alpine

RUN apk add --no-cache \
   git \
   wget \
   build-base \
   pcre-dev \
   zlib-dev \
   openssl-dev \
   brotli-dev \
   cmake \
   linux-headers

RUN mkdir -p /usr/local/src && \
   cd /usr/local/src && \
   git clone --recursive https://github.com/google/ngx_brotli.git && \
   cd ngx_brotli && \
   git submodule update --init && \
   cd deps/brotli && \
   mkdir out && cd out && \
   cmake -DCMAKE_BUILD_TYPE=Release -DBROTLI_SHARED_LIBS=OFF .. && \
   make && \
   make install

RUN cd /usr/local/src && \
   wget http://nginx.org/download/nginx-${NGINX_VERSION}.tar.gz && \
   tar zxf nginx-${NGINX_VERSION}.tar.gz

RUN cd /usr/local/src/nginx-${NGINX_VERSION} && \
   ./configure --with-compat --add-dynamic-module=/usr/local/src/ngx_brotli && \
   make modules

RUN mkdir -p /etc/nginx/modules && \
   mkdir -p /etc/nginx/modules-enabled && \
   cp /usr/local/src/nginx-${NGINX_VERSION}/objs/ngx_http_brotli_filter_module.so \
      /usr/local/src/nginx-${NGINX_VERSION}/objs/ngx_http_brotli_static_module.so \
      /etc/nginx/modules/

RUN apk del git wget build-base && \
   rm -rf /usr/local/src/*

COPY nginx.conf /etc/nginx/nginx.conf