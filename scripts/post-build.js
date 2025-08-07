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

// Create html directory if it doesn't exist
if (!existsSync(htmlDir)) {
  mkdirSync(htmlDir, { recursive: true });
  console.log('✅ Created html directory');
}

// Copy index.html to html/index.html if it doesn't exist
if (existsSync(indexHtmlSrc) && !existsSync(indexHtmlDest)) {
  copyFileSync(indexHtmlSrc, indexHtmlDest);
  console.log('✅ Copied index.html to html directory');
}

// Add CSP meta tag for Electron security
if (existsSync(indexHtmlSrc)) {
  let htmlContent = readFileSync(indexHtmlSrc, 'utf8');
  
  // Check if CSP is already present
  if (!htmlContent.includes('Content-Security-Policy')) {
    // Add CSP meta tag after the charset meta tag
    const cspPolicy = `<meta http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' ws: wss:; worker-src 'self' blob:;">`;
    
    htmlContent = htmlContent.replace(
      '<meta name="viewport" content="width=device-width, initial-scale=1.0">',
      `<meta name="viewport" content="width=device-width, initial-scale=1.0">\n  ${cspPolicy}`
    );
    
    writeFileSync(indexHtmlSrc, htmlContent);
    console.log('✅ Added CSP meta tag to index.html');
  }
}

console.log('🚀 Post-build setup complete!');