import { parse, serialize } from 'parse5';
import { readFileSync, writeFileSync } from 'node:fs';

// Maintenance only. A normal website build never reads outside website/.
const source = new URL('../../apps/desktop/index.html', import.meta.url);
const target = new URL('../src/data/help.json', import.meta.url);
const document = parse(readFileSync(source, 'utf8'));
const attr = (node, key) => node.attrs?.find(a => a.name === key)?.value;
const walk = (node) => [node, ...(node.childNodes || []).flatMap(walk)];
const text = (node) => node.nodeName === '#text' ? node.value : (node.childNodes || []).map(text).join('');
const articles = walk(document).filter(n => n.tagName === 'article' && attr(n, 'id')?.startsWith('help-'));
if (articles.length !== 9) throw new Error('Help topic structure changed; review the website mirror.');
const topics = articles.map(article => {
  const heading = article.childNodes.find(n => n.tagName === 'h3');
  const title = text(heading);
  article.childNodes = article.childNodes.filter(n => n !== heading);
  for (const node of walk(article)) {
    if (node.tagName === 'h4') { node.tagName = 'h2'; node.nodeName = 'h2'; }
    if (node.tagName === 'kbd') {
      const shortcut = attr(node, 'data-shortcut');
      const modifier = node.attrs?.some(a => a.name === 'data-shortcut-modifier');
      if (shortcut || modifier) node.childNodes = [{ nodeName: '#text', value: shortcut ? `Ctrl / ⌘ + ${shortcut}` : 'Ctrl (Windows/Linux) or ⌘ (macOS)', parentNode: node }];
      node.attrs = [];
    }
  }
  return { slug: attr(article, 'id').replace('help-', ''), title, html: serialize(article).trim() };
});
const output = JSON.stringify(topics, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (readFileSync(target, 'utf8') !== output) throw new Error('App Help changed. Run npm --prefix website run sync:help and review the result.');
  console.log('All 9 website guide topics match app Help.');
} else {
  writeFileSync(target, output);
  console.log('Synced all 9 app Help topics.');
}
