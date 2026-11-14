// Long-lived localhost server of the synthetic workload (stands in for a lane's API server).
// node server.js <port-file>   GET /blob?n=<bytes>   GET /work?ms=<ms>   POST /sink
const http = require('http');
const fs = require('fs');
const crypto = require('crypto');
const server = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  if (req.method === 'POST') {
    let n = 0;
    req.on('data', (d) => { n += d.length; });
    req.on('end', () => { res.end(String(n)); });
    return;
  }
  if (url.pathname === '/work') {
    const end = Date.now() + Number(url.searchParams.get('ms') || 20);
    while (Date.now() < end) crypto.createHash('sha256').update('x' + Date.now()).digest();
    return res.end('ok');
  }
  res.end(Buffer.alloc(Number(url.searchParams.get('n') || 1024), 97));
});
server.listen(0, '127.0.0.1', () => {
  fs.writeFileSync(process.argv[2], String(server.address().port));
});
