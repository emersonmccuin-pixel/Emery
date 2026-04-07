export function getStoredValue(key: string, legacyKey?: string): string | null {
  const current = localStorage.getItem(key);
  if (current !== null) {
    return current;
  }

  if (!legacyKey) {
    return null;
  }

  const legacy = localStorage.getItem(legacyKey);
  if (legacy !== null) {
    localStorage.setItem(key, legacy);
    localStorage.removeItem(legacyKey);
  }
  return legacy;
}

export function setStoredValue(key: string, value: string, legacyKey?: string) {
  localStorage.setItem(key, value);
  if (legacyKey) {
    localStorage.removeItem(legacyKey);
  }
}

export function removeStoredValue(key: string, legacyKey?: string) {
  localStorage.removeItem(key);
  if (legacyKey) {
    localStorage.removeItem(legacyKey);
  }
}
