import { useCallback, useEffect, useState } from 'react';
import { getSettings } from '../lib/api';
import type { ProviderListing, SettingsResponse } from '../lib/api';

export function useSettings() {
  const [settings, setSettings] = useState<SettingsResponse | null>(null);

  useEffect(() => {
    getSettings()
      .then(setSettings)
      .catch(() => setSettings(null));
  }, []);

  const handleSettingsSaved = useCallback((next: SettingsResponse) => {
    setSettings(next);
  }, []);

  // A provider listing refreshed on demand (the fetch button) replaces that
  // provider's entry, or appends it when the provider was not listed yet.
  const handleListing = useCallback((listing: ProviderListing) => {
    setSettings((previous) => {
      if (!previous) return previous;
      const exists = previous.listings.some((item) => item.provider === listing.provider);
      const listings = exists
        ? previous.listings.map((item) => (item.provider === listing.provider ? listing : item))
        : [...previous.listings, listing];
      return { ...previous, listings };
    });
  }, []);

  return { settings, handleSettingsSaved, handleListing };
}
