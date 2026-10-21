/**
 * Package script — builds the extension and creates a distributable .zip file.
 *
 * Output: dist/open-infra-extension.zip containing manifest.json + main.js (flat, no directory prefix)
 */

import { readFileSync, writeFileSync, existsSync } from 'fs';
import { join } from 'path';
import { deflateRawSync } from 'zlib';

const rootDir = join(import.meta.dirname, '..');
const distDir = join(rootDir, 'dist');
const mainJs = join(distDir, 'main.js');
const manifestJson = join(rootDir, 'manifest.json');

if (!existsSync(mainJs)) {
  console.error('dist/main.js not found. Run "npm run build" first.');
  process.exit(1);
}

if (!existsSync(manifestJson)) {
  console.error('manifest.json not found in project root.');
  process.exit(1);
}

const files = [
  { name: 'manifest.json', data: readFileSync(manifestJson) },
  { name: 'main.js', data: readFileSync(mainJs) },
];

/**
 * Build a minimal ZIP file from an array of { name, data } entries.
 */
function createZip(entries) {
  const localHeaders = [];
  const centralHeaders = [];
  let offset = 0;

  for (const entry of entries) {
    const nameBytes = Buffer.from(entry.name, 'utf-8');
    const compressed = deflateRawSync(entry.data);
    const crc = crc32(entry.data);

    // Local file header (30 bytes + name + compressed data)
    const local = Buffer.alloc(30 + nameBytes.length + compressed.length);
    local.writeUInt32LE(0x04034b50, 0); // signature
    local.writeUInt16LE(20, 4);          // version needed
    local.writeUInt16LE(0, 6);           // flags
    local.writeUInt16LE(8, 8);           // compression: deflate
    local.writeUInt16LE(0, 10);          // mod time
    local.writeUInt16LE(0, 12);          // mod date
    local.writeUInt32LE(crc, 14);        // crc-32
    local.writeUInt32LE(compressed.length, 18); // compressed size
    local.writeUInt32LE(entry.data.length, 22); // uncompressed size
    local.writeUInt16LE(nameBytes.length, 26);  // file name length
    local.writeUInt16LE(0, 28);          // extra field length
    nameBytes.copy(local, 30);
    compressed.copy(local, 30 + nameBytes.length);
    localHeaders.push(local);

    // Central directory header (46 bytes + name)
    const central = Buffer.alloc(46 + nameBytes.length);
    central.writeUInt32LE(0x02014b50, 0); // signature
    central.writeUInt16LE(20, 4);          // version made by
    central.writeUInt16LE(20, 6);          // version needed
    central.writeUInt16LE(0, 8);           // flags
    central.writeUInt16LE(8, 10);          // compression: deflate
    central.writeUInt16LE(0, 12);          // mod time
    central.writeUInt16LE(0, 14);          // mod date
    central.writeUInt32LE(crc, 16);        // crc-32
    central.writeUInt32LE(compressed.length, 20); // compressed size
    central.writeUInt32LE(entry.data.length, 24); // uncompressed size
    central.writeUInt16LE(nameBytes.length, 28);  // file name length
    central.writeUInt16LE(0, 30);          // extra field length
    central.writeUInt16LE(0, 32);          // file comment length
    central.writeUInt16LE(0, 34);          // disk number start
    central.writeUInt16LE(0, 36);          // internal attributes
    central.writeUInt32LE(0, 38);          // external attributes
    central.writeUInt32LE(offset, 42);     // local header offset
    nameBytes.copy(central, 46);
    centralHeaders.push(central);

    offset += local.length;
  }

  const centralDirOffset = offset;
  const centralDirSize = centralHeaders.reduce((s, b) => s + b.length, 0);

  // End of central directory record (22 bytes)
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);     // signature
  eocd.writeUInt16LE(0, 4);              // disk number
  eocd.writeUInt16LE(0, 6);              // disk with central dir
  eocd.writeUInt16LE(entries.length, 8); // entries on this disk
  eocd.writeUInt16LE(entries.length, 10);// total entries
  eocd.writeUInt32LE(centralDirSize, 12);// central dir size
  eocd.writeUInt32LE(centralDirOffset, 16); // central dir offset
  eocd.writeUInt16LE(0, 20);             // comment length

  return Buffer.concat([...localHeaders, ...centralHeaders, eocd]);
}

/**
 * CRC-32 computation (standard ZIP/PNG polynomial).
 */
function crc32(buf) {
  let crc = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    crc ^= buf[i];
    for (let j = 0; j < 8; j++) {
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

const zipBuffer = createZip(files);
const outputPath = join(distDir, 'open-infra-extension.zip');
writeFileSync(outputPath, zipBuffer);

console.log(`Created ${outputPath} (${zipBuffer.length} bytes)`);
console.log(`  - manifest.json (${files[0].data.length} bytes)`);
console.log(`  - main.js (${files[1].data.length} bytes)`);
