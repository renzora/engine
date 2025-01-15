const path = require('path');
const TerserPlugin = require('terser-webpack-plugin');

module.exports = {
  entry: path.resolve(__dirname, '../client/assets/js/engine/index.js'),
  output: {
    filename: 'renzora.min.js',
    path: path.resolve(__dirname, '../client/assets/js/'),
  },
  mode: 'production', // Automatically enables minification
  watch: true, // Watch for changes
  watchOptions: {
    aggregateTimeout: 300, // Delay rebuild after the first change
    poll: 1000, // Check for changes every second
    ignored: /node_modules/, // Ignore unnecessary directories
  },
  optimization: {
    minimize: true, // Explicitly enable minification
    minimizer: [
      new TerserPlugin({
        terserOptions: {
          compress: true,
          mangle: true,
        },
      }),
    ],
  },
};