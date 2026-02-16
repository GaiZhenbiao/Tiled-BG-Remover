export async function generateMockTile(width: number, height: number, rowIndex: number, colIndex: number): Promise<Blob> {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d');
  
  if (!ctx) throw new Error("Could not get canvas context");

  // Fill with random noise
  const imageData = ctx.createImageData(width, height);
  const data = imageData.data;
  for (let i = 0; i < data.length; i += 4) {
    const val = Math.floor(Math.random() * 255);
    data[i] = val;     // R
    data[i + 1] = val; // G
    data[i + 2] = val; // B
    data[i + 3] = 255; // Alpha
  }
  ctx.putImageData(imageData, 0, 0);

  // Draw giant coordinate text
  ctx.fillStyle = "black";
  ctx.font = `bold ${Math.min(width, height) / 3}px sans-serif`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(`(${rowIndex},${colIndex})`, width / 2, height / 2);
  
  // Add a border to verify tile edges
  ctx.strokeStyle = "red";
  ctx.lineWidth = 10;
  ctx.strokeRect(0, 0, width, height);

  return new Promise((resolve, reject) => {
    canvas.toBlob(blob => {
      if (blob) resolve(blob);
      else reject(new Error("Canvas to Blob failed"));
    }, 'image/png');
  });
}
