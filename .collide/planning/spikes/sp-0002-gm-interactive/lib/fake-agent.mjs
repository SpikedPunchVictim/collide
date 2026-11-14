// A stand-in for an agent: prints a numbered line every 200 ms, echoes each stdin line as a numbered "recv", exits on "quit".
import readline from 'node:readline';
let seq = 0;
const tick = setInterval(() => console.log(`seq=${seq++} t=${Date.now()}`), 200);
const rl = readline.createInterface({ input: process.stdin });
let got = 0;
rl.on('line', (l) => { console.log(`recv=${got++} text=${l} t=${Date.now()}`); if (l === 'quit') { clearInterval(tick); process.exit(7); } });
rl.on('close', () => { console.log(`stdin-closed t=${Date.now()}`); clearInterval(tick); process.exit(9); });
