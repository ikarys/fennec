import type { AttachmentInput } from './api';

export const IMAGE_TYPES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp', 'image/bmp', 'image/svg+xml'];
export const MAX_IMAGE_BYTES = 5 * 1024 * 1024;
export const MAX_ATTACHMENTS = 10;

/** A file pinned to an idea: any type, kept as a data URL. */
export interface FileAttachmentInput {
  filename: string;
  mimeType: string;
  size: number;
  data: string;
}

function readAsDataUrl(file: File): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

/** Any file to a data URL: images and files alike, capped like images. */
export async function readFiles(
  files: File[],
  current: number,
): Promise<{ attachments: FileAttachmentInput[]; error: string | null }> {
  const remaining = MAX_ATTACHMENTS - current;
  if (remaining <= 0) {
    return { attachments: [], error: '10 fichiers au maximum' };
  }

  const valid: File[] = [];
  for (const file of files) {
    if (file.size > MAX_IMAGE_BYTES) {
      return { attachments: [], error: `« ${file.name || 'fichier'} » dépasse 5 Mo` };
    }
    valid.push(file);
  }

  if (valid.length > remaining) {
    return { attachments: [], error: '10 fichiers au maximum' };
  }

  const attachments: FileAttachmentInput[] = [];
  for (const file of valid) {
    const data = await readAsDataUrl(file);
    attachments.push({
      filename: file.name || 'fichier',
      mimeType: file.type || 'application/octet-stream',
      size: file.size,
      data,
    });
  }

  return { attachments, error: null };
}

export async function readImages(files: File[], current: number): Promise<{ attachments: AttachmentInput[]; error: string | null }> {
  const remaining = MAX_ATTACHMENTS - current;
  if (remaining <= 0) {
    return { attachments: [], error: '10 images au maximum' };
  }

  const valid: File[] = [];
  for (const file of files) {
    if (!IMAGE_TYPES.includes(file.type)) {
      return { attachments: [], error: `« ${file.name || 'image.png'} » n’est pas une image` };
    }
    if (file.size > MAX_IMAGE_BYTES) {
      return { attachments: [], error: `« ${file.name || 'image.png'} » dépasse 5 Mo` };
    }
    valid.push(file);
  }

  if (valid.length > remaining) {
    return { attachments: [], error: '10 images au maximum' };
  }

  const attachments: AttachmentInput[] = [];
  for (const file of valid) {
    const data = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(file);
    });
    attachments.push({ filename: file.name || 'image.png', data });
  }

  return { attachments, error: null };
}
