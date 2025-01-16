const esbuild = require('esbuild');
const path = require('path');
const { minify } = require('terser');
const fs = require('fs').promises;

async function build() {
  const ctx = await esbuild.context({
    entryPoints: {
      'js/renzora.min': '../client/assets/js/engine/index.js'
    },
    outdir: path.resolve(__dirname, '../client/assets'),
    bundle: true,
    minify: true,
    loader: {
      '.png': 'file',
      '.jpg': 'file',
      '.gif': 'file',
      '.svg': 'file'
    },
    assetNames: 'img/[name]-[hash]',
    publicPath: '../',
    target: ['es2020'],
    drop: ['console', 'debugger'],
    minifyIdentifiers: true,
    minifySyntax: true,
    minifyWhitespace: true,
    treeShaking: true,
    charset: 'utf8',
    mangleProps: /^_/,
    pure: ['console.log'],
    format: 'esm',
    platform: 'browser'
  });

  // Initial build
  await ctx.rebuild();

  // Watch for changes
  await ctx.watch();
  console.log('Watching for changes...');

  // Additional Terser minification after each build
  const jsPath = path.resolve(__dirname, '../client/assets/js/renzora.min.js');
  
  async function performTerserMinification() {
    try {
      const code = await fs.readFile(jsPath, 'utf8');
      const minified = await minify(code, {
        compress: {
          dead_code: true,
          drop_console: true,
          drop_debugger: true,
          keep_fargs: false,
          passes: 3,
          pure_funcs: ['console.log'],
          unsafe: true,
          unsafe_math: true
        },
        mangle: {
          properties: {
            regex: /^_/
          }
        },
        format: {
          comments: false
        }
      });

      await fs.writeFile(jsPath, minified.code);
      console.log('Additional minification completed, waiting for tailwind...');
    } catch (err) {
      console.error('Terser minification failed:', err);
    }
  }

  // Run initial minification
  await performTerserMinification();

  // Set up file watcher for additional minification
  const chokidar = require('chokidar');
  chokidar.watch(jsPath).on('change', async () => {
    await performTerserMinification();
  });
}

build().catch((err) => {
  console.error('Build failed:', err);
  process.exit(1);
});