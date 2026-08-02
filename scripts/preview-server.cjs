const http = require('http');
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve('C:/Users/longq/Desktop/route (1)');
const srv = http.createServer((req, res) => {
  // Strip query string so path computation doesn't get tripped up
  // by `?fresh=1` style cache-busters.
  const urlNoQuery = (req.url || '/').split('?')[0];
  let p = urlNoQuery === '/' ? '/route-logo.html' : urlNoQuery;
  const file = path.normalize(path.join(ROOT, decodeURIComponent(p)));
  if (!file.startsWith(ROOT) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
    res.writeHead(404, { 'content-type': 'text/plain' });
    res.end('not found: ' + file);
    return;
  }
  const ext = path.extname(file);
  const ct = { '.html': 'text/html', '.js': 'application/javascript', '.css': 'text/css' }[ext] || 'text/plain';
  res.writeHead(200, { 'content-type': ct, 'cache-control': 'no-store' });
  fs.createReadStream(file).pipe(res);
});
srv.listen(14920, '127.0.0.1', () => console.log('ready on 14920'));
