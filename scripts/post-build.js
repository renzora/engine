#!/usr/bin/env node

import { copyFileSync, mkdirSync, existsSync, readFileSync, writeFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const projectRoot = resolve(__dirname, '..');
const clientDistDir = resolve(projectRoot, 'dist', 'client');
const htmlDir = resolve(clientDistDir, 'html');
const indexHtmlSrc = resolve(clientDistDir, 'index.html');
const indexHtmlDest = resolve(htmlDir, 'index.html');

console.log('🔧 Running post-build setup...');

if (!existsSync(htmlDir)) {
  mkdirSync(htmlDir, { recursive: true });
  console.log('✅ Created html directory');
}

if (existsSync(indexHtmlSrc) && !existsSync(indexHtmlDest)) {
  copyFileSync(indexHtmlSrc, indexHtmlDest);
  console.log('✅ Copied index.html to html directory');
}

if (existsSync(indexHtmlSrc)) {
  let htmlContent = readFileSync(indexHtmlSrc, 'utf8');
  
  if (!htmlContent.includes('Content-Security-Policy')) {
    const cspPolicy = `<meta http-equiv="Content-Security-Policy" content="default-src * 'unsafe-inline' 'unsafe-eval'; script-src * 'unsafe-inline' 'unsafe-eval'; connect-src * 'unsafe-inline'; img-src * data: blob:; style-src * 'unsafe-inline';">`;
    
    htmlContent = htmlContent.replace(
      '<meta name="viewport" content="width=device-width, initial-scale=1.0">',
      `<meta name="viewport" content="width=device-width, initial-scale=1.0">\n  ${cspPolicy}`
    );
    
    writeFileSync(indexHtmlSrc, htmlContent);
    console.log('✅ Added CSP meta tag to index.html');
  }
}

console.log('🚀 Post-build setup complete!');