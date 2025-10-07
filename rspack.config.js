import { defineConfig } from '@rspack/cli'
import { rspack } from '@rspack/core'
import { resolve } from 'node:path'
import fs from 'fs'
import refresh from 'solid-refresh/babel'

const isProduction = process.env.NODE_ENV === 'production';

const config = {
  mode: isProduction ? 'production' : 'development',
  entry: {
    main: './src/entry-client.jsx',
  },
  
  experiments: {
    css: true,
  },
  
  infrastructureLogging: {
    level: 'warn',
    debug: false,
    colors: false,
  },
  
  devtool: false,
  
  stats: 'normal',
  
  resolve: {
    alias: {
      '@': resolve(import.meta.dirname, 'src')
    },
    extensions: ['.js', '.jsx', '.ts', '.tsx', '.json'],
    fullySpecified: false
  },
  
  module: {
    rules: [
      {
        test: /\.(jsx|tsx)$/,
        use: [
          {
            loader: 'babel-loader',
            options: {
              presets: [
                ['solid', {
                  generate: 'dom',
                  hydratable: false,
                  dev: process.env.NODE_ENV !== 'production'
                }]
              ],
              plugins: !isProduction ? [refresh] : [],
            },
          },
        ],
      },
      {
        test: /\.css$/,
        use: [
          {
            loader: 'postcss-loader',
            options: {
              postcssOptions: {
                plugins: [
                  '@tailwindcss/postcss'
                ]
              }
            }
          }
        ],
        type: 'css'
      }
    ],
  },
  
  plugins: [
    new rspack.HtmlRspackPlugin({
      template: './src/index.html',
    }),
    new rspack.DefinePlugin({
      'process.env.NODE_ENV': JSON.stringify(isProduction ? 'production' : 'development'),
      'import.meta.env.DEV': JSON.stringify(!isProduction),
      'import.meta.env.PROD': JSON.stringify(isProduction),
      'import.meta.env.MODE': JSON.stringify(isProduction ? 'production' : 'development'),
      '__DEV__': JSON.stringify(!isProduction),
    }),
    !isProduction && new rspack.HotModuleReplacementPlugin(),
    // Bridge server runs standalone on port 3001
  ].filter(Boolean),
  
  optimization: {
    splitChunks: {
      chunks: 'all',
      cacheGroups: {
        babylon: {
          test: /[\\/]node_modules[\\/]@babylonjs[\\/]/,
          name: 'babylon',
          chunks: 'all',
          priority: 30,
        },
        solidjs: {
          test: /[\\/]node_modules[\\/]solid-js/,
          name: 'solid',
          chunks: 'all',
          priority: 25,
          reuseExistingChunk: true,
        },
        vendor: {
          test: /[\\/]node_modules[\\/]/,
          name: 'vendor',
          chunks: 'all',
          priority: 10,
        },
      },
    },
    minimize: isProduction,
  },
  
  performance: {
    hints: false,
    maxAssetSize: 10000000,
    maxEntrypointSize: 10000000,
  },
  
  output: {
    path: resolve(import.meta.dirname, 'dist'),
    filename: isProduction ? 'assets/[name]-[contenthash].js' : 'assets/[name].js',
    chunkFilename: isProduction ? 'assets/[name]-[contenthash].js' : 'assets/[name].js',
    assetModuleFilename: isProduction ? 'assets/[name]-[contenthash][ext]' : 'assets/[name][ext]',
    clean: true,
    publicPath: '/',
  }
};

// Only add devServer configuration in development
if (!isProduction) {
  config.devServer = {
    port: 3000,
    host: 'localhost',
    hot: true,
    liveReload: false,
    historyApiFallback: true,
    static: {
      directory: resolve(import.meta.dirname, 'dist'),
      publicPath: '/',
    },
    headers: {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, PATCH, OPTIONS',
      'Access-Control-Allow-Headers': 'X-Requested-With, Content-Type, Authorization',
    },
    client: {
      logging: 'warn',
      progress: false,
    },
    setupExitSignals: false,
    compress: false,
    devMiddleware: {
      stats: 'none',
      writeToDisk: false,
    },
    allowedHosts: 'all',
    onListening: function (devServer) {
      if (!devServer) {
        throw new Error('webpack-dev-server is not defined');
      }
      const { port } = devServer.server.address();
      const protocol = devServer.server.listening && devServer.options.server?.type === 'https' ? 'https' : 'http';
      const url = `${protocol}://localhost:${port}`;
      
      console.log(`
██████╗ ███████╗███╗   ██╗███████╗ ██████╗ ██████╗  █████╗ 
██╔══██╗██╔════╝████╗  ██║╚══███╔╝██╔═══██╗██╔══██╗██╔══██╗
██████╔╝█████╗  ██╔██╗ ██║  ███╔╝ ██║   ██║██████╔╝███████║
██╔══██╗██╔══╝  ██║╚██╗██║ ███╔╝  ██║   ██║██╔══██╗██╔══██║
██║  ██║███████╗██║ ╚████║███████╗╚██████╔╝██║  ██║██║  ██║
╚═╝  ╚═╝╚══════╝╚═╝  ╚═══╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝

███████╗███╗   ██╗ ██████╗ ██╗███╗   ██╗███████╗
██╔════╝████╗  ██║██╔════╝ ██║████╗  ██║██╔════╝
█████╗  ██╔██╗ ██║██║  ███╗██║██╔██╗ ██║█████╗  
██╔══╝  ██║╚██╗██║██║   ██║██║██║╚██╗██║██╔══╝  
███████╗██║ ╚████║╚██████╔╝██║██║ ╚████║███████╗
╚══════╝╚═╝  ╚═══╝ ╚═════╝ ╚═╝╚═╝  ╚═══╝╚══════╝

🚀 Development server running at: \x1b]8;;${url}\x1b\\${url}\x1b]8;;\x1b\\
`);
    },
    server: (() => {
      try {
        const keyPath = resolve(import.meta.dirname, 'localhost+2-key.pem')
        const certPath = resolve(import.meta.dirname, 'localhost+2.pem')
        
        if (fs.existsSync(keyPath) && fs.existsSync(certPath)) {
          return {
            type: 'https',
            options: {
              key: fs.readFileSync(keyPath),
              cert: fs.readFileSync(certPath),
            }
          }
        }
      } catch {
        console.warn('HTTPS certificates not found, falling back to HTTP')
      }
      return 'http'
    })(),
  };
}

export default defineConfig(config);