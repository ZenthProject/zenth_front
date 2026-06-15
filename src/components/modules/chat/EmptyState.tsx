import { MessageSquare, Lock, Shield } from "lucide-react";

interface EmptyStateProps {
  contactName?: string;
}

export function EmptyState({ contactName }: EmptyStateProps) {
  return (
    <div className="flex-1 flex flex-col items-center justify-center p-8 text-center">
      <div className="relative mb-6">
        <div className="h-20 w-20 rounded-full bg-gradient-to-br from-indigo-600/20 to-purple-600/20 flex items-center justify-center">
          <MessageSquare className="h-10 w-10 text-indigo-400" />
        </div>
        <div className="absolute -bottom-1 -right-1 h-8 w-8 rounded-full bg-emerald-600/20 flex items-center justify-center">
          <Lock className="h-4 w-4 text-emerald-400" />
        </div>
      </div>

      <h3 className="text-xl font-semibold text-gray-100 mb-2">
        {contactName ? `Conversation avec ${contactName}` : "Nouvelle conversation"}
      </h3>

      <p className="text-gray-400 max-w-sm mb-6">
        Les messages sont chiffres de bout en bout. Personne en dehors de cette conversation ne peut les lire.
      </p>

      <div className="flex items-center gap-6 text-xs text-gray-500">
        <div className="flex items-center gap-1.5">
          <Lock className="h-3.5 w-3.5 text-emerald-500" />
          <span>E2E Chiffre</span>
        </div>
        <div className="flex items-center gap-1.5">
          <Shield className="h-3.5 w-3.5 text-indigo-400" />
          <span>Post-quantique</span>
        </div>
      </div>
    </div>
  );
}
