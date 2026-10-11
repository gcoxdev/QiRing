import { readdir, readFile, access, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parse } from 'parse5';

const root = fileURLToPath(new URL('../dist/', import.meta.url));
async function files(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  return (await Promise.all(entries.map(entry => entry.isDirectory() ? files(path.join(directory, entry.name)) : path.join(directory, entry.name)))).flat();
}
function* walk(node) { yield node; for (const child of node.childNodes || []) yield* walk(child); }
const pages = new Map();
for (const file of (await files(root)).filter(file => file.endsWith('.html'))) {
  pages.set(file, [...walk(parse(await readFile(file, 'utf8')))]);
}
for (const [file, nodes] of pages) {
  if (nodes.filter(node => node.tagName === 'h1').length !== 1) throw new Error(`${file}: expected one h1`);
  for (const node of nodes) {
    const attrs = Object.fromEntries((node.attrs || []).map(attr => [attr.name, attr.value]));
    if (node.tagName === 'img' && !Object.hasOwn(attrs, 'alt')) throw new Error(`${file}: missing image description`);
    for (const ref of [attrs.href, attrs.src].filter(Boolean)) {
      if (/^(https?:|mailto:|data:)/.test(ref)) continue;
      const base = `https://check.invalid/${path.relative(root, file)}`;
      const url = new URL(ref, base);
      let target = path.join(root, decodeURIComponent(url.pathname));
      if (url.pathname.endsWith('/')) target = path.join(target, 'index.html');
      await access(target).catch(() => { throw new Error(`${file}: broken reference ${ref}`); });
      if (url.hash && pages.has(target) && !pages.get(target).some(node => node.attrs?.some(attr => attr.name === 'id' && attr.value === decodeURIComponent(url.hash.slice(1))))) throw new Error(`${file}: missing anchor ${ref}`);
    }
  }
}
if (process.env.SITE_URL) {
  const site = new URL(process.env.SITE_URL);
  if (!['http:', 'https:'].includes(site.protocol) || site.pathname !== '/' || site.search || site.hash) throw new Error('SITE_URL must be an HTTP(S) origin, such as https://qiring.example');
  const urls = [...pages.keys()].filter(file => !file.endsWith('/404.html')).map(file => new URL(path.relative(root, file).replace(/index\.html$/, ''), site).href);
  const xml = value => value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('"', '&quot;');
  await writeFile(path.join(root, 'sitemap.xml'), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">${urls.map(url => `<url><loc>${xml(url)}</loc></url>`).join('')}</urlset>\n`);
  await writeFile(path.join(root, 'robots.txt'), `User-agent: *\nAllow: /\nSitemap: ${new URL('sitemap.xml', site).href}\n`);
}
console.log(`Checked ${pages.size} static pages, local links, anchors, and images.`);
