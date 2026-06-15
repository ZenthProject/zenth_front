import React from 'react';
import { Loader2, Lock } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface TorButtonProps {
  isLoading?: boolean;
  loadingText?: string;
  children: React.ReactNode;
  Icon?: React.ElementType;
  className?: string;
  [key: string]: any;
}

const TorButton = ({
  isLoading = false,
  loadingText = 'Chargement...',
  children,
  Icon = Lock,
  className = '',
  ...rest
}: TorButtonProps) => {
  return (
    <Button
      disabled={isLoading}
      size="lg"
      className={`w-full font-semibold ${className}`}
      {...rest}
    >
      {isLoading ? (
        <span className="flex items-center justify-center gap-2">
          <Loader2 className="h-4 w-4 animate-spin" />
          {loadingText}
        </span>
      ) : (
        <span className="flex items-center justify-center gap-2">
          <Icon className="h-4 w-4" />
          {children}
        </span>
      )}
    </Button>
  );
};

export default TorButton;
