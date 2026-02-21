type GenerateImageOptions = {
  apiBaseUrl?: string;
  fullImageBlob?: Blob | null;
};

function normalizeApiBaseUrl(apiBaseUrl?: string): string {
  const fallback = 'https://generativelanguage.googleapis.com';
  const base = (apiBaseUrl || fallback).trim();
  if (!base) return fallback;
  return base.replace(/\/+$/, '');
}

async function blobToInlineData(blob: Blob): Promise<{ mime_type: string; data: string }> {
  const reader = new FileReader();
  reader.readAsDataURL(blob);
  const base64Data = await new Promise<string>((resolve) => {
    reader.onloadend = () => resolve(reader.result as string);
  });
  const [prefix, data] = base64Data.split(',');
  const mimeType = prefix?.split(';')[0]?.split(':')[1] || blob.type || 'image/png';
  return {
    mime_type: mimeType,
    data
  };
}

async function convertBlobToJpeg(blob: Blob, quality = 0.92, maxSide = 1024): Promise<Blob> {
  if (typeof window === 'undefined' || typeof document === 'undefined') return blob;

  try {
    const bitmap = await createImageBitmap(blob);
    const longestSide = Math.max(bitmap.width, bitmap.height);
    const scale = longestSide > maxSide ? maxSide / longestSide : 1;
    const targetWidth = Math.max(1, Math.round(bitmap.width * scale));
    const targetHeight = Math.max(1, Math.round(bitmap.height * scale));

    if (blob.type === 'image/jpeg' && scale === 1) {
      bitmap.close();
      return blob;
    }

    const canvas = document.createElement('canvas');
    canvas.width = targetWidth;
    canvas.height = targetHeight;

    const ctx = canvas.getContext('2d');
    if (!ctx) {
      bitmap.close();
      return blob;
    }

    // Flatten any alpha against white before JPEG encoding.
    ctx.fillStyle = '#FFFFFF';
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = 'high';
    ctx.drawImage(bitmap, 0, 0, targetWidth, targetHeight);
    bitmap.close();

    const jpegBlob = await new Promise<Blob | null>((resolve) => {
      canvas.toBlob(resolve, 'image/jpeg', quality);
    });
    return jpegBlob ?? blob;
  } catch {
    return blob;
  }
}

function extractResponseParts(data: any): any[] {
  return data?.candidates?.[0]?.content?.parts || [];
}

function extractFirstText(data: any): string {
  const parts = extractResponseParts(data);
  for (const part of parts) {
    if (typeof part?.text === 'string' && part.text.trim()) {
      return part.text.trim();
    }
  }
  return '';
}

function isLikelySubject(candidate: string): boolean {
  if (!candidate) return false;
  if (!/[a-z]/i.test(candidate)) return false;
  if (candidate.length < 3) return false;
  if (/^[a-z]$/i.test(candidate)) return false;

  const words = candidate.split(/\s+/).filter(Boolean);
  if (words.length === 0 || words.length > 4) return false;
  if (words.some((w) => w.length <= 1)) return false;

  const invalid = new Set(['unknown', 'none', 'n a', 'n/a', 'subject', 'object']);
  if (invalid.has(candidate.toLowerCase())) return false;

  return true;
}

function normalizeSubject(raw: string): string {
  if (!raw) return '';

  const segments = raw
    .split(/\r?\n|[;,|]/)
    .map((s) => s.trim())
    .filter(Boolean);

  for (const seg of segments) {
    const cleaned = seg
      .toLowerCase()
      .replace(/^["'`([{]+|["'`)\]}]+$/g, '')
      .replace(/^\(?[a-z0-9]\)?[.):\-]\s*/i, '')
      .replace(/^(the|a|an)\s+/i, '')
      .replace(/[^a-z0-9\s-]/g, ' ')
      .replace(/\s+/g, ' ')
      .trim();

    if (isLikelySubject(cleaned)) {
      return cleaned;
    }
  }

  const fallback = raw
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  return isLikelySubject(fallback) ? fallback : '';
}

function extractFirstInlineImage(data: any): Blob | null {
  const parts = extractResponseParts(data);
  for (const part of parts) {
    const inlineData = part?.inline_data || part?.inlineData;
    if (inlineData?.data) {
      return b64toBlob(
        inlineData.data,
        inlineData.mime_type || inlineData.mimeType || 'image/png'
      );
    }
  }
  return null;
}

export async function generateImage(
  imageBlob: Blob | null,
  prompt: string,
  model: string,
  apiKey: string,
  options: GenerateImageOptions = {}
): Promise<Blob> {
  const baseUrl = normalizeApiBaseUrl(options.apiBaseUrl);
  const url = `${baseUrl}/v1beta/models/${model}:generateContent?key=${apiKey}`;

  const parts: any[] = [];
  if (options.fullImageBlob) {
    const fullInline = await blobToInlineData(options.fullImageBlob);
    parts.push({ text: "Reference full image context (for global consistency):" });
    parts.push({ inline_data: fullInline });
  }
  if (imageBlob) {
    const tileInline = await blobToInlineData(imageBlob);
    parts.push({ text: "Target tile to generate/edit:" });
    parts.push({ inline_data: tileInline });
  }
  parts.push({ text: prompt });

  const payload = {
    contents: [
      {
        role: "user",
        parts
      }
    ],
    generationConfig: {
      temperature: 0.35,
      maxOutputTokens: 2048,
      responseModalities: ["IMAGE"],
      seed: Math.floor(Math.random() * 2147483647) + Date.now() % 1000000
    }
  };

  const response = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload)
  });

  if (!response.ok) {
    const errText = await response.text();
    throw new Error(`API Error ${response.status}: ${errText}`);
  }

  const data = await response.json();
  const image = extractFirstInlineImage(data);
  if (image) return image;

  const snippet = JSON.stringify(data).substring(0, 240);
  throw new Error(`Model did not return an image. Response: ${snippet}...`);
}

export async function detectMainSubject(
  imageBlob: Blob,
  apiKey: string,
  apiBaseUrl?: string
): Promise<string> {
  const baseUrl = normalizeApiBaseUrl(apiBaseUrl);
  const model = 'gemini-3-flash-preview';
  const url = `${baseUrl}/v1beta/models/${model}:generateContent?key=${apiKey}`;

  const jpegBlob = await convertBlobToJpeg(imageBlob);
  const imageInline = await blobToInlineData(jpegBlob);

  async function runSubjectPrompt(prompt: string, maxTokens = 48): Promise<string> {
    const payload = {
      contents: [
        {
          role: "user",
          parts: [
            { inline_data: imageInline },
            { text: prompt }
          ]
        }
      ],
      generationConfig: {
        temperature: 0.1,
        maxOutputTokens: maxTokens,
        responseModalities: ["TEXT"]
      }
    };

    const response = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });

    if (!response.ok) {
      const errText = await response.text();
      throw new Error(`Subject detection API Error ${response.status}: ${errText}`);
    }

    const data = await response.json();
    return extractFirstText(data);
  }

  const first = normalizeSubject(
    await runSubjectPrompt(
      "Identify the primary foreground object. Return one concise common noun phrase in English (1-3 words), lowercase, no explanation."
    )
  );
  if (first) return first;

  try {
    const retry = normalizeSubject(
      await runSubjectPrompt(
        "Return exactly one common object noun phrase in English, 1-3 words, lowercase. Never return a single letter or symbol. Example outputs: bicycle, road bicycle, person, shoe.",
        32
      )
    );
    if (retry) return retry;
  } catch {
    // Preserve UX by falling back when retry parsing fails after first pass was invalid.
  }

  return 'main subject';
}

function b64toBlob(b64Data: string, contentType = '', sliceSize = 512) {
  const byteCharacters = atob(b64Data);
  const byteArrays = [];

  for (let offset = 0; offset < byteCharacters.length; offset += sliceSize) {
    const slice = byteCharacters.slice(offset, offset + sliceSize);
    const byteNumbers = new Array(slice.length);
    for (let i = 0; i < slice.length; i++) {
      byteNumbers[i] = slice.charCodeAt(i);
    }
    const byteArray = new Uint8Array(byteNumbers);
    byteArrays.push(byteArray);
  }

  return new Blob(byteArrays, { type: contentType });
}
