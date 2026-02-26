import { browser } from '$app/environment';

export type ImageGenerationModel = {
  nickname: string;
  name: string;
};

export const GEMINI_MODELS_STORAGE_KEY = 'gemini_models';

export const DEFAULT_IMAGE_GENERATION_MODELS: ImageGenerationModel[] = [
  { nickname: 'Nano Banana', name: 'gemini-2.5-flash-image' },
  { nickname: 'Nano Banana Pro', name: 'gemini-3-pro-image-preview' },
  { nickname: 'Nano Banana 2', name: 'gemini-3.1-flash-image-preview' }
];

const LEGACY_DEFAULT_NICKNAMES: Record<string, string[]> = {
  'gemini-2.5-flash-image': ['Flash 2.5'],
  'gemini-3-pro-image-preview': ['Pro 3 Preview']
};

function normalizeModel(input: unknown): ImageGenerationModel | null {
  if (!input || typeof input !== 'object') return null;
  const maybe = input as { nickname?: unknown; name?: unknown };
  const nickname = typeof maybe.nickname === 'string' ? maybe.nickname.trim() : '';
  const name = typeof maybe.name === 'string' ? maybe.name.trim() : '';
  if (!nickname || !name) return null;
  return { nickname, name };
}

function cloneDefaults(): ImageGenerationModel[] {
  return DEFAULT_IMAGE_GENERATION_MODELS.map((model) => ({ ...model }));
}

function migrateLegacyDefaultNicknames(models: ImageGenerationModel[]): ImageGenerationModel[] {
  return models.map((model) => {
    const defaultModel = DEFAULT_IMAGE_GENERATION_MODELS.find((item) => item.name === model.name);
    if (!defaultModel) return model;
    const legacyNicknames = LEGACY_DEFAULT_NICKNAMES[model.name] || [];
    if (legacyNicknames.includes(model.nickname)) {
      return { ...model, nickname: defaultModel.nickname };
    }
    return model;
  });
}

export function sanitizeModelList(input: unknown): ImageGenerationModel[] {
  if (!Array.isArray(input)) return cloneDefaults();
  const seen = new Set<string>();
  const result: ImageGenerationModel[] = [];
  for (const item of input) {
    const model = normalizeModel(item);
    if (!model) continue;
    const key = model.name.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(model);
  }
  return result.length > 0 ? result : cloneDefaults();
}

export function readImageGenerationModels(): ImageGenerationModel[] {
  if (!browser) return cloneDefaults();
  try {
    const raw = localStorage.getItem(GEMINI_MODELS_STORAGE_KEY);
    if (!raw) return cloneDefaults();
    const sanitized = sanitizeModelList(JSON.parse(raw));
    const migrated = migrateLegacyDefaultNicknames(sanitized);
    if (JSON.stringify(migrated) !== JSON.stringify(sanitized)) {
      localStorage.setItem(GEMINI_MODELS_STORAGE_KEY, JSON.stringify(migrated));
    }
    return migrated;
  } catch {
    return cloneDefaults();
  }
}

export function writeImageGenerationModels(models: ImageGenerationModel[]): ImageGenerationModel[] {
  const sanitized = sanitizeModelList(models);
  if (browser) {
    localStorage.setItem(GEMINI_MODELS_STORAGE_KEY, JSON.stringify(sanitized));
  }
  return sanitized;
}

export function ensureSelectedModelName(
  models: ImageGenerationModel[],
  preferredName: string
): string {
  const normalizedPreferred = (preferredName || '').trim();
  const list = sanitizeModelList(models);
  if (normalizedPreferred && list.some((model) => model.name === normalizedPreferred)) {
    return normalizedPreferred;
  }
  return list[0].name;
}
