// TODO: Settings hook
//
// This hook will provide settings utilities:
// - useSettings(): Access to settings context
// - useSetting(key): Get single setting value
// - useUpdateSetting(): Update setting helper
//
// For now, this re-exports the context hook.
// Future: Add additional settings-related hooks here.

export { useSettings } from '@/contexts/SettingsContext';
