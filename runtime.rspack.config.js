const { defineConfig } = require('@rspack/cli');
const path = require('path');

module.exports = defineConfig({
  entry: {
    main: './src/runtime/main.jsx'
  },
  output: {
    path: path.resolve(__dirname, 'dist-runtime'),
    filename: '[name].js',
    clean: true
  },
  resolve: {
    extensions: ['.jsx', '.js', '.json'],
    alias: {
      '@': path.resolve(__dirname, 'src')
    }
  },
  module: {
    rules: [
      {
        test: /\.jsx?$/,
        use: {
          loader: 'babel-loader',
          options: {
            presets: ['babel-preset-solid']
          }
        },
        exclude: /node_modules/
      },
      {
        test: /\.css$/,
        use: ['postcss-loader']
      }
    ]
  },
  plugins: [],
  devServer: {
    port: 3002,
    hot: true,
    historyApiFallback: true
  },
  mode: process.env.NODE_ENV || 'development'
});