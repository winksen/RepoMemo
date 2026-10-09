/** Limits the picker enforces before anything is sent; the server checks them again. */
export const AVATAR_INPUT_TYPES = ["image/png", "image/jpeg", "image/webp"];
export const AVATAR_INPUT_MAX_BYTES = 10 * 1024 * 1024;
const AVATAR_SIDE = 256;

/**
 * Turns a chosen file into a small square picture: centre-cropped, scaled to 256 px and
 * redrawn on a canvas. Redrawing drops metadata (such as GPS position) and anything hidden
 * in the original file, and keeps the upload to a few tens of kilobytes.
 */
export async function prepareAvatar(file: File): Promise<Blob> {
  if (!AVATAR_INPUT_TYPES.includes(file.type)) throw new Error("Choose a PNG, JPEG or WebP image.");
  if (file.size > AVATAR_INPUT_MAX_BYTES) throw new Error("Choose an image smaller than 10 MB.");
  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(file);
  } catch {
    throw new Error("That file could not be read as an image.");
  }
  try {
    const side = Math.min(bitmap.width, bitmap.height);
    if (side < 64) throw new Error("Choose an image at least 64 pixels wide and tall.");
    const canvas = document.createElement("canvas");
    canvas.width = AVATAR_SIDE;
    canvas.height = AVATAR_SIDE;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("This browser cannot prepare the picture.");
    context.imageSmoothingQuality = "high";
    context.drawImage(bitmap, (bitmap.width - side) / 2, (bitmap.height - side) / 2, side, side, 0, 0, AVATAR_SIDE, AVATAR_SIDE);
    for (const type of ["image/webp", "image/jpeg"]) {
      const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, type, 0.9));
      if (blob && blob.type === type) return blob;
    }
    throw new Error("This browser cannot prepare the picture.");
  } finally {
    bitmap.close();
  }
}
