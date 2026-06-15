import { ReactNode } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { LucideIcon } from "lucide-react";

interface SettingsCardProps {
  icon: LucideIcon;
  title: string;
  description?: string;
  children: ReactNode;
  variant?: "default" | "danger";
}

export function SettingsCard({
  icon: Icon,
  title,
  description,
  children,
  variant = "default"
}: SettingsCardProps) {
  const borderClass = variant === "danger"
    ? "border-2 border-destructive/50"
    : "border-border";

  const iconColor = variant === "danger"
    ? "text-destructive"
    : "text-accent-secondary";

  return (
    <Card className={`bg-card ${borderClass}`}>
      <CardHeader>
        <div className="flex items-center gap-2">
          <Icon className={`h-5 w-5 ${iconColor}`} />
          <CardTitle className="text-card-foreground">{title}</CardTitle>
        </div>
        {description && (
          <CardDescription className={variant === "danger" ? "text-destructive" : "text-muted-foreground"}>
            {description}
          </CardDescription>
        )}
      </CardHeader>
      <CardContent className="space-y-4">
        {children}
      </CardContent>
    </Card>
  );
}
