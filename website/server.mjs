import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
const files = {
  '/img_black.png': ['img_black.png', 'image/png'],
  '/img_white.png': ['img_white.png', 'image/png'],
  '/': ['index.html', 'text/html; charset=utf-8'],
  '/index.html': ['index.html', 'text/html; charset=utf-8'],
  '/styles.css': ['styles.css', 'text/css; charset=utf-8'],
  '/app.js': ['app.js', 'text/javascript; charset=utf-8'],
  '/favicon.svg': ['favicon.svg', 'image/svg+xml'],
  '/docs/api/': ['docs/api/index.html', 'text/html; charset=utf-8'],
  '/docs/api/index.html': ['docs/api/index.html', 'text/html; charset=utf-8'],
  '/docs/docs.css': ['docs/docs.css', 'text/css; charset=utf-8'],
};
createServer(async (req, res) => {
  const file = files[new URL(req.url, 'http://localhost').pathname];
  if (!file) { res.writeHead(404); res.end('Not found'); return; }
  try { const body = await readFile(new URL(file[0], import.meta.url)); res.writeHead(200, { 'Content-Type': file[1] }); res.end(body); }
  catch { res.writeHead(500); res.end('Unable to read file'); }
}).listen(4173, '127.0.0.1', () => console.log('Local: http://127.0.0.1:4173'));
