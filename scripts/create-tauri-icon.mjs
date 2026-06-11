import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const iconPath = resolve("apps/desktop/src-tauri/icons/icon.ico");
const size = 32;
const xorSize = size * size * 4;
const maskStride = Math.ceil(size / 32) * 4;
const maskSize = maskStride * size;
const bitmapHeaderSize = 40;
const imageSize = bitmapHeaderSize + xorSize + maskSize;
const fileSize = 6 + 16 + imageSize;
const buffer = Buffer.alloc(fileSize);

let offset = 0;

function u16(value) {
  buffer.writeUInt16LE(value, offset);
  offset += 2;
}

function u32(value) {
  buffer.writeUInt32LE(value, offset);
  offset += 4;
}

function i32(value) {
  buffer.writeInt32LE(value, offset);
  offset += 4;
}

// ICONDIR
u16(0);
u16(1);
u16(1);

// ICONDIRENTRY
buffer[offset++] = size;
buffer[offset++] = size;
buffer[offset++] = 0;
buffer[offset++] = 0;
u16(1);
u16(32);
u32(imageSize);
u32(22);

// BITMAPINFOHEADER. ICO stores double height for XOR + AND masks.
u32(bitmapHeaderSize);
i32(size);
i32(size * 2);
u16(1);
u16(32);
u32(0);
u32(xorSize);
i32(0);
i32(0);
u32(0);
u32(0);

const pixelOffset = offset;

for (let y = 0; y < size; y += 1) {
  for (let x = 0; x < size; x += 1) {
    const row = size - 1 - y;
    const idx = pixelOffset + (row * size + x) * 4;
    const border = x < 2 || y < 2 || x >= size - 2 || y >= size - 2;
    const spine = x >= 7 && x <= 11 && y >= 6 && y <= 25;
    const pageTop = x >= 12 && x <= 24 && y >= 7 && y <= 11;
    const pageMid = x >= 12 && x <= 22 && y >= 14 && y <= 17;
    const pageLow = x >= 12 && x <= 25 && y >= 21 && y <= 24;

    let r = 14;
    let g = 111;
    let b = 104;

    if (border) {
      r = 32;
      g = 35;
      b = 31;
    }

    if (spine || pageTop || pageMid || pageLow) {
      r = 255;
      g = 248;
      b = 233;
    }

    buffer[idx] = b;
    buffer[idx + 1] = g;
    buffer[idx + 2] = r;
    buffer[idx + 3] = 255;
  }
}

await mkdir(dirname(iconPath), { recursive: true });
await writeFile(iconPath, buffer);

console.log(`Created ${iconPath}`);
