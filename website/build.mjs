import { mkdir, copyFile, cp } from 'node:fs/promises';
await mkdir(new URL('./dist/', import.meta.url), { recursive: true });
for (const file of ['index.html', 'styles.css', 'app.js', 'favicon.svg', 'img_black.png', 'img_white.png', 'CNAME']) {
  await copyFile(new URL(file, import.meta.url), new URL(`dist/${file}`, import.meta.url));
}
await cp(new URL('./docs/', import.meta.url), new URL('./dist/docs/', import.meta.url), { recursive: true });
console.log('Built website/dist');
