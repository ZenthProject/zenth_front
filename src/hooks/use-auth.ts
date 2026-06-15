// TODO: Authentication hook
//
// This hook will provide authentication utilities:
// - useAuth(): Access to auth context
// - useRequireAuth(): Redirect if not authenticated
// - useAuthCheck(): Check session validity
//
// For now, this re-exports the context hook.
// Future: Add additional auth-related hooks here.

export { useAuth } from '@/contexts/AuthContext';
