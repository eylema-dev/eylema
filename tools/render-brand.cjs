#!/usr/bin/env node
// SVG files are canonical. sharp is only a development-time rasterizer.
const fs = require('node:fs/promises');
const path = require('node:path');
const sharp = require('sharp');

const root = path.resolve(__dirname, '..', 'assets', 'eylema-logo');
const font = "'Helvetica Neue', Arial, sans-serif";
const svg = (width, height, title, body) => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}" role="img" aria-labelledby="title"><title id="title">${title}</title>${body}</svg>\n`;

async function main() {
  const files = ['logo', 'logo-dark', 'logo-mono', 'logo-icon', 'github-avatar'];
  for (const name of files) {
    await sharp(path.join(root, `${name}.svg`), { density: 288 })
      .resize({ width: name.startsWith('logo') && name !== 'logo-icon' ? 1600 : 1024 })
      .png().toFile(path.join(root, `${name}.png`));
  }
  // Inline the exact source geometry, avoiding external images or linked fonts.
  const inner = async (name) => (await fs.readFile(path.join(root, `${name}.svg`), 'utf8'))
    .replace(/^<svg[^>]*>/, '').replace(/<\/svg>\s*$/, '')
    .replace(/\s*<(title|desc)\b[^>]*>[\s\S]*?<\/\1>/g, '');
  const light = await inner('logo');
  const dark = await inner('logo-dark');
  const avatar = await inner('github-avatar');
  const icon = await inner('logo-icon');
  const preview = svg(1600, 1080, 'Eylema brand preview — light, dark and avatar applications', `
    <rect width="1600" height="1080" fill="#F7F6F2"/>
    <text x="96" y="86" fill="#17252B" font-family="${font}" font-size="22" letter-spacing="4">EYLEMA / BRAND IDENTITY</text>
    <text x="1504" y="86" text-anchor="end" fill="#17252B" font-family="${font}" font-size="22">eylema.dev</text>
    <g transform="translate(268 125) scale(1.4)">${light}</g>
    <path d="M96 560H1504" stroke="#D8DCDA"/>
    <rect x="96" y="624" width="882" height="324" rx="16" fill="#17252B"/>
    <g transform="translate(154 646) scale(1)">${dark}</g>
    <g transform="translate(1050 624) scale(1.265625)">${avatar}</g>
    <text x="96" y="1008" fill="#17252B" font-family="${font}" font-size="22">ACTION ORANGE  #EE5638</text>
    <text x="620" y="1008" fill="#17252B" font-family="${font}" font-size="22">INK  #17252B</text>
    <g transform="translate(1410 994) scale(.0625)">${icon}</g>
    <g transform="translate(1450 986) scale(.125)">${icon}</g>
  `);
  const social = svg(1280, 640, 'Eylema — agentic AI for continuous development and operations', `
    <rect width="1280" height="640" fill="#17252B"/>
    <g transform="translate(166 70) scale(1.25)">${dark}</g>
    <text x="640" y="450" text-anchor="middle" fill="#FFFFFF" font-family="${font}" font-size="28">Continuous development. Continuous operations.</text>
    <text x="640" y="538" text-anchor="middle" fill="#EE5638" font-family="${font}" font-size="24" letter-spacing="2">eylema.dev</text>
  `);
  for (const [name, source, width] of [['brand-preview', preview, 1600], ['github-social', social, 1280]]) {
    await fs.writeFile(path.join(root, `${name}.svg`), source);
    await sharp(Buffer.from(source)).resize({ width }).png().toFile(path.join(root, `${name}.png`));
  }
  for (const size of [16, 32, 48]) {
    await sharp(path.join(root, 'logo-icon.svg'), { density: 288 }).resize(size, size)
      .png().toFile(path.join(root, `favicon-${size}.png`));
  }
  await sharp(path.join(root, 'github-avatar.svg'), { density: 288 }).resize(180, 180)
    .png().toFile(path.join(root, 'apple-touch-icon.png'));
  console.log('Rendered 7 SVG assets, 3 favicon sizes and the Apple touch icon.');
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
