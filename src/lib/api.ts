export async function generateImage(imageBlob: Blob, prompt: string, model: string, apiKey: string): Promise<Blob> {
  const reader = new FileReader();
  reader.readAsDataURL(imageBlob);
  const base64Data = await new Promise<string>((resolve) => {
    reader.onloadend = () => resolve(reader.result as string);
  });
  // Strip prefix data:image/...;base64,
  const base64Image = base64Data.split(',')[1];
  const mimeType = base64Data.split(';')[0].split(':')[1];

  const url = `https://generativelanguage.googleapis.com/v1beta/models/${model}:generateContent?key=${apiKey}`;
  
  const payload = {
    contents: [{
      parts: [
        { text: prompt },
        { inline_data: { mime_type: mimeType, data: base64Image } }
      ]
    }],
    generationConfig: {
      temperature: 0.4,
      maxOutputTokens: 2048, 
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
  
  // Try to parse image from response
  // 1. Check for inline_data (standard multimodal response for images)
  const parts = data.candidates?.[0]?.content?.parts || [];
  for (const part of parts) {
    if (part.inline_data && part.inline_data.data) {
      return b64toBlob(part.inline_data.data, part.inline_data.mime_type || 'image/png');
    }
  }
  
  // 2. Fallback: Check if text contains a base64 string (sometimes models output text)
  // This is hacky but might work for some experimental models
  for (const part of parts) {
    if (part.text && part.text.length > 1000) { // arbitrary length check
       // Try to find base64 pattern?
       // For now, assume if the model doesn't return inline_data, it failed for our purpose.
    }
  }
  
  console.warn("No image found in response", data);
  throw new Error("Model did not return an image. Check if the model supports image output.");
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
