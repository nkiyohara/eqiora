import type { UnstructuredFieldDescriptor } from "./unstructured-field-protocol";

export type CanvasPoint = Readonly<{ x: number; y: number }>;
type Rgb = readonly [red: number, green: number, blue: number];

const MAX_CANVAS_PIXEL_COUNT = 4_194_304;
const MAX_RASTER_SAMPLE_COUNT = 32_000_000;

export function drawUnstructuredP1Field(
  canvas: HTMLCanvasElement,
  descriptor: UnstructuredFieldDescriptor,
  coordinates: Float64Array,
  triangles: Uint32Array,
  values: Float64Array,
): void {
  requireDrawableContract(descriptor, coordinates, triangles, values);
  const [width, height] = boundedCanvasDimensions(
    canvas.clientWidth,
    canvas.clientHeight,
    window.devicePixelRatio,
  );
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (context === null) throw new Error("Canvas 2D context is unavailable.");
  context.clearRect(0, 0, width, height);
  const point = (vertex: number): CanvasPoint => {
    const offset = vertex * 2;
    const x = coordinates[offset];
    const y = coordinates[offset + 1];
    if (x === undefined || y === undefined || !Number.isFinite(x) || !Number.isFinite(y)) {
      throw new Error("Triangle references an invalid coordinate.");
    }
    return {
      x: normalizedCoordinate(x, descriptor.domain.boundsM[0]) * width,
      y: (1 - normalizedCoordinate(y, descriptor.domain.boundsM[1])) * height,
    };
  };
  requireBoundedRasterWork(descriptor, triangles, point, width, height);
  const image = context.createImageData(width, height);
  const palette = scalarPalette();
  for (let triangle = 0; triangle < descriptor.mesh.triangleCount; triangle += 1) {
    const a = triangles[triangle * 3];
    const b = triangles[triangle * 3 + 1];
    const c = triangles[triangle * 3 + 2];
    if (a === undefined || b === undefined || c === undefined) {
      throw new Error("Triangle connectivity is incomplete.");
    }
    const pa = point(a);
    const pb = point(b);
    const pc = point(c);
    const va = values[a];
    const vb = values[b];
    const vc = values[c];
    if (va === undefined || vb === undefined || vc === undefined) {
      throw new Error("Triangle references an invalid P1 coefficient.");
    }
    rasterizeP1Triangle(
      image,
      [pa, pb, pc],
      [va, vb, vc],
      descriptor.field.minimum,
      descriptor.field.maximum,
      palette,
    );
  }
  context.putImageData(image, 0, 0);
  for (let triangle = 0; triangle < descriptor.mesh.triangleCount; triangle += 1) {
    const a = triangles[triangle * 3];
    const b = triangles[triangle * 3 + 1];
    const c = triangles[triangle * 3 + 2];
    if (a === undefined || b === undefined || c === undefined) continue;
    const pa = point(a);
    const pb = point(b);
    const pc = point(c);
    context.beginPath();
    context.moveTo(pa.x, pa.y);
    context.lineTo(pb.x, pb.y);
    context.lineTo(pc.x, pc.y);
    context.closePath();
    context.strokeStyle = "rgba(14, 42, 36, 0.18)";
    context.lineWidth = Math.max(0.5, window.devicePixelRatio * 0.5);
    context.stroke();
  }
}

function boundedCanvasDimensions(
  clientWidth: number,
  clientHeight: number,
  devicePixelRatio: number,
): readonly [width: number, height: number] {
  const pixelRatio =
    Number.isFinite(devicePixelRatio) && devicePixelRatio > 0 ? Math.min(devicePixelRatio, 4) : 1;
  const width = Math.max(1, Math.round(clientWidth * pixelRatio));
  const height = Math.max(1, Math.round(clientHeight * pixelRatio));
  const pixelCount = width * height;
  if (pixelCount <= MAX_CANVAS_PIXEL_COUNT) return [width, height];
  const scale = Math.sqrt(MAX_CANVAS_PIXEL_COUNT / pixelCount);
  return [Math.max(1, Math.floor(width * scale)), Math.max(1, Math.floor(height * scale))];
}

function requireBoundedRasterWork(
  descriptor: UnstructuredFieldDescriptor,
  triangles: Uint32Array,
  point: (vertex: number) => CanvasPoint,
  width: number,
  height: number,
): void {
  let samples = 0;
  for (let triangle = 0; triangle < descriptor.mesh.triangleCount; triangle += 1) {
    const a = triangles[triangle * 3];
    const b = triangles[triangle * 3 + 1];
    const c = triangles[triangle * 3 + 2];
    if (a === undefined || b === undefined || c === undefined) {
      throw new Error("Triangle connectivity is incomplete.");
    }
    const bounds = rasterBounds([point(a), point(b), point(c)], width, height);
    samples += (bounds.upperX - bounds.lowerX + 1) * (bounds.upperY - bounds.lowerY + 1);
    if (samples > MAX_RASTER_SAMPLE_COUNT) {
      throw new Error("Triangle projection exceeds the bounded presentation work budget.");
    }
  }
}

export function interpolateP1Triangle(
  point: CanvasPoint,
  points: readonly [CanvasPoint, CanvasPoint, CanvasPoint],
  values: readonly [number, number, number],
): number | null {
  const [a, b, c] = points;
  const weights = barycentricWeights(point.x, point.y, a, b, c, triangleDenominator(a, b, c));
  return weights === null
    ? null
    : weights[0] * values[0] + weights[1] * values[1] + weights[2] * values[2];
}

export function normalizedCoordinate(
  value: number,
  [lower, upper]: readonly [number, number],
): number {
  return clamp((value - lower) / (upper - lower), 0, 1);
}

function rasterizeP1Triangle(
  image: ImageData,
  points: readonly [CanvasPoint, CanvasPoint, CanvasPoint],
  values: readonly [number, number, number],
  minimum: number,
  maximum: number,
  palette: readonly Rgb[],
): void {
  const [a, b, c] = points;
  const { denominator, lowerX, upperX, lowerY, upperY } = rasterBounds(
    points,
    image.width,
    image.height,
  );
  const edgeTolerance = 1e-10;
  for (let y = lowerY; y <= upperY; y += 1) {
    for (let x = lowerX; x <= upperX; x += 1) {
      const weights = barycentricWeights(x + 0.5, y + 0.5, a, b, c, denominator, edgeTolerance);
      if (weights === null) continue;
      const value = weights[0] * values[0] + weights[1] * values[1] + weights[2] * values[2];
      const colour =
        palette[Math.round(normalizedScalar(value, minimum, maximum) * (palette.length - 1))];
      if (colour === undefined) continue;
      const offset = (y * image.width + x) * 4;
      image.data[offset] = colour[0];
      image.data[offset + 1] = colour[1];
      image.data[offset + 2] = colour[2];
      image.data[offset + 3] = 255;
    }
  }
}

function rasterBounds(
  points: readonly [CanvasPoint, CanvasPoint, CanvasPoint],
  width: number,
  height: number,
): Readonly<{
  denominator: number;
  lowerX: number;
  upperX: number;
  lowerY: number;
  upperY: number;
}> {
  const [a, b, c] = points;
  const denominator = triangleDenominator(a, b, c);
  if (!Number.isFinite(denominator) || Math.abs(denominator) <= Number.EPSILON) {
    throw new Error("Triangle projection is degenerate.");
  }
  return {
    denominator,
    lowerX: clamp(Math.floor(Math.min(a.x, b.x, c.x)), 0, width - 1),
    upperX: clamp(Math.ceil(Math.max(a.x, b.x, c.x)), 0, width - 1),
    lowerY: clamp(Math.floor(Math.min(a.y, b.y, c.y)), 0, height - 1),
    upperY: clamp(Math.ceil(Math.max(a.y, b.y, c.y)), 0, height - 1),
  };
}

function triangleDenominator(a: CanvasPoint, b: CanvasPoint, c: CanvasPoint): number {
  return (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
}

function barycentricWeights(
  x: number,
  y: number,
  a: CanvasPoint,
  b: CanvasPoint,
  c: CanvasPoint,
  denominator: number,
  edgeTolerance = 0,
): readonly [number, number, number] | null {
  if (!Number.isFinite(denominator) || Math.abs(denominator) <= Number.EPSILON) return null;
  const weightA = ((b.y - c.y) * (x - c.x) + (c.x - b.x) * (y - c.y)) / denominator;
  const weightB = ((c.y - a.y) * (x - c.x) + (a.x - c.x) * (y - c.y)) / denominator;
  const weightC = 1 - weightA - weightB;
  return weightA < -edgeTolerance || weightB < -edgeTolerance || weightC < -edgeTolerance
    ? null
    : [weightA, weightB, weightC];
}

function requireDrawableContract(
  descriptor: UnstructuredFieldDescriptor,
  coordinates: Float64Array,
  triangles: Uint32Array,
  values: Float64Array,
): void {
  if (
    coordinates.length !== descriptor.mesh.vertexCount * 2 ||
    triangles.length !== descriptor.mesh.triangleCount * 3 ||
    values.length !== descriptor.field.valueCount
  ) {
    throw new Error("Materialized arrays differ from the accepted unstructured descriptor.");
  }
  if (
    coordinates.some((value) => !Number.isFinite(value)) ||
    values.some((value) => !Number.isFinite(value)) ||
    triangles.some((vertex) => vertex >= descriptor.mesh.vertexCount)
  ) {
    throw new Error("Materialized unstructured arrays violate finite/index bounds.");
  }
}

function scalarPalette(): readonly Rgb[] {
  return Array.from({ length: 256 }, (_, index) => scalarRgb(index / 255));
}

function normalizedScalar(value: number, minimum: number, maximum: number): number {
  return minimum === maximum ? 0.5 : clamp((value - minimum) / (maximum - minimum), 0, 1);
}

function scalarRgb(normalized: number): Rgb {
  const hue = 164 - normalized * 152;
  const saturation = 0.66;
  const lightness = 0.36 + normalized * 0.18;
  const chroma = (1 - Math.abs(2 * lightness - 1)) * saturation;
  const hueSector = hue / 60;
  const second = chroma * (1 - Math.abs((hueSector % 2) - 1));
  const [red, green, blue] =
    hueSector < 1
      ? [chroma, second, 0]
      : hueSector < 2
        ? [second, chroma, 0]
        : hueSector < 3
          ? [0, chroma, second]
          : hueSector < 4
            ? [0, second, chroma]
            : hueSector < 5
              ? [second, 0, chroma]
              : [chroma, 0, second];
  const offset = lightness - chroma / 2;
  return [
    Math.round((red + offset) * 255),
    Math.round((green + offset) * 255),
    Math.round((blue + offset) * 255),
  ];
}

function clamp(value: number, lower: number, upper: number): number {
  return Math.min(upper, Math.max(lower, value));
}
