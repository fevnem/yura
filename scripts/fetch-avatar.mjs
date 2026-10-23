#!/usr/bin/env node
/**
 * Fetch a default character.
 *
 * VRM files carry their own licence inside the file, and most "free" models
 * forbid redistribution. So the avatar is downloaded on the user's machine, not
 * committed here. Anything you drop into `ui/public/assets/models/` is picked
 * up automatically — this script exists only to make the first launch pretty.
 *
 *   node scripts/fetch-avatar.mjs                 # default sample
 *   node scripts/fetch-avatar.mjs <url> [name.vrm] # your own
 */

import { createWriteStream } from 'node:fs';
import { mkdir, readdir, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { pipeline } from 'node:stream/promises';
import { Readable } from 'node:stream';

const here = dirname(fileURLToPath(import.meta.url));
const modelsDir = resolve(here, '..', 'ui', 'public', 'assets', 'models');

/**
 * VRM Consortium's sample avatars are CC0-by-policy and safe to download.
 * If the URL is unavailable, the app still runs with the procedural spirit
 * form, so a failed download is never fatal.
 */
const DEFAULT_SOURCES = [
  {
    url: 'https://raw.githubusercontent.com/vrm-c/UniVRM/master/Tests/Models/Alicia_vrm-0.51/AliciaSolid_vrm-0.51.vrm',
    name: 'AliciaSolid.vrm',
  },
];

async function main() {
  await mkdir(modelsDir, { recursive: true });

  const [, , argUrl, argName] = process.argv;
  const sources = argUrl ? [{ url: argUrl, name: argName || 'avatar.vrm' }] : DEFAULT_SOURCES;

  for (const source of sources) {
    const target = join(modelsDir, source.name);
    try {
      process.stdout.write(`↓ ${source.name} … `);
      const response = await fetch(source.url, { redirect: 'follow' });
      if (!response.ok || !response.body) {
        throw new Error(`HTTP ${response.status}`);
      }
      await pipeline(Readable.fromWeb(response.body), createWriteStream(target));
      console.log('ok');
    } catch (error) {
      console.log(`skipped (${error.message})`);
    }
  }

  await writeManifest();
}

/** The renderer cannot list a directory, so the manifest is built here. */
async function writeManifest() {
  const files = (await readdir(modelsDir)).filter((f) => f.toLowerCase().endsWith('.vrm')).sort();
  const manifest = {
    generated: new Date().toISOString(),
    models: files.map((file) => ({
      file,
      label: file.replace(/\.vrm$/i, '').replace(/[_-]+/g, ' '),
    })),
  };
  await writeFile(join(modelsDir, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`manifest.json: ${files.length} model(s)`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
