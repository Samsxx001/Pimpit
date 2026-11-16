import fs from 'node:fs';
const root = new URL('../data/', import.meta.url);
const en = new Set(fs.readFileSync(new URL('english-raw.txt', root), 'utf8').split(/\r?\n/).map(l => l.split(' ')[0].split('(')[0].toLowerCase()).filter(w => /^[a-z]{1,32}$/.test(w)));
const th = new Set(fs.readFileSync(new URL('thai-raw.txt', root), 'utf8').replace(/^\uFEFF/, '').split(/\r?\n/).map(w => w.trim()).filter(w => w.length >= 2 && w.length <= 96 && /^[\u0e01-\u0e4e]+$/.test(w)));
for (const w of ['สวัสดี','ครับ','ค่ะ','ขอบคุณ','ภาษาไทย','ภาษาอังกฤษ','ทดสอบ','คอมพิวเตอร์']) th.add(w);
for (const [name, words] of [['english', en], ['thai', th]]) {
  const data = [...words].sort().join('\n') + '\n';
  fs.writeFileSync(new URL(name + '.txt', root), data);
  console.log(`${name}: ${words.size} words, ${Buffer.byteLength(data)} bytes`);
}
