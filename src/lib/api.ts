export async function generateImage(imageBlob: Blob | null, prompt: string, model: string, apiKey: string): Promise<Blob> {
  const url = `https://generativelanguage.googleapis.com/v1beta/models/${model}:generateContent?key=${apiKey}`;
  
  let parts: any[] = [{ text: prompt }];

  if (imageBlob) {
      const reader = new FileReader();
      reader.readAsDataURL(imageBlob);
      const base64Data = await new Promise<string>((resolve) => {
        reader.onloadend = () => resolve(reader.result as string);
      });
      // Strip prefix data:image/...;base64,
      const base64Image = base64Data.split(',')[1];
      const mimeType = base64Data.split(';')[0].split(':')[1];
      
      parts = [
        { inline_data: { mime_type: mimeType, data: base64Image } },
        { text: prompt }
      ];
  }
  
  const payload = {
    contents: [{
      role: "user",
      parts: parts
    }],
    generationConfig: {
      temperature: 0.4,
      maxOutputTokens: 2048,
      responseModalities: ["IMAGE"]
    }
  };

  console.log(`Sending request to ${model} with prompt: "${prompt}" (Has Image: ${!!imageBlob})`);

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
  const responseParts = data.candidates?.[0]?.content?.parts || [];
  for (const part of responseParts) {
    const inlineData = part.inline_data || part.inlineData;
    if (inlineData && inlineData.data) {
      return b64toBlob(inlineData.data, inlineData.mime_type || inlineData.mimeType || 'image/png');
    }
  }
  
  // 2. Fallback: Check if text contains a base64 string (sometimes models output text)
  // This is hacky but might work for some experimental models
  for (const part of responseParts) {
    if (part.text && part.text.length > 1000) { // arbitrary length check
       // Try to find base64 pattern?
       // For now, assume if the model doesn't return inline_data, it failed for our purpose.
    }
  }
  
  console.warn("No image found in response", JSON.stringify(data, null, 2));
  // Include a snippet of the JSON in the error message for debugging
  const snippet = JSON.stringify(data).substring(0, 200);
  throw new Error(`Model did not return an image. Response: ${snippet}...`);
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
