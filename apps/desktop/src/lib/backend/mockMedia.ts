// Invented media files for the mock backends (the browser preview and the screenshots): a sticker as a transparent
// PNG drawn on a canvas, so the sticker card shows a picture; never a real file.
import { MIME } from "../generated/msgkinds";

/** A round, smiling sticker, `size` pixels square, its colour picked by `seed`. */
export async function mockSticker(seed: number, size = 160): Promise<ArrayBuffer> {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = size;
  const g = canvas.getContext("2d")!;
  const r = size / 2;
  g.fillStyle = `hsl(${(seed * 47) % 360}, 70%, 60%)`;
  g.beginPath();
  g.arc(r, r, r * 0.9, 0, Math.PI * 2);
  g.fill();
  g.strokeStyle = "#1b1b1b";
  g.lineWidth = size / 24;
  g.beginPath();
  g.arc(r, r * 1.05, r * 0.45, 0.15 * Math.PI, 0.85 * Math.PI);
  g.stroke();
  g.fillStyle = "#1b1b1b";
  for (const x of [0.7, 1.3]) {
    g.beginPath();
    g.arc(r * x, r * 0.8, size / 18, 0, Math.PI * 2);
    g.fill();
  }
  const blob = await new Promise<Blob | null>((done) => canvas.toBlob(done, MIME.PNG));
  return blob ? new Response(blob).arrayBuffer() : new ArrayBuffer(0);
}
