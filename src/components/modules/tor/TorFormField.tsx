import React from 'react';
import { Path, FieldValues, UseFormReturn, ControllerRenderProps } from 'react-hook-form';
import { FormField, FormItem, FormControl, FormMessage, FormDescription } from '@/components/ui/form';
import { Input } from '@/components/ui/input';
import { Eye, EyeOff, User, Lock, Mail, LucideIcon } from 'lucide-react';
import { cn } from '@/lib/utils';

const DEFAULT_ACCENT = 'purple-400';

interface TorFormFieldProps<T extends FieldValues> {
  form: UseFormReturn<T>;
  name: Path<T>; 
  placeholder?: string;
  type?: React.HTMLInputTypeAttribute;
  accentColor?: string;
  Icon?: LucideIcon;
  className?: string; 
  description?: React.ReactNode; 
}

function TorFormField<T extends FieldValues>({
  form,
  name,
  placeholder = '',
  type = 'text',
  accentColor = DEFAULT_ACCENT,
  Icon,
  className,
  description,
}: TorFormFieldProps<T>) {

  const [showPassword, setShowPassword] = React.useState(false);
  const isPassword = type === 'password';

  const renderInput = (field: ControllerRenderProps<T, Path<T>>) => {
    const baseInputClasses = cn(
      'bg-gray-700 border-gray-600 text-gray-50 placeholder:text-gray-400',
      `focus:border-${accentColor}`,
      'focus-visible:ring-0',
      Icon ? 'pl-10' : 'pl-4',
      isPassword ? 'pr-10' : 'pr-4',
    );

    return (
      <div className="relative flex items-center">
        {Icon && (
          <Icon className="absolute left-3 h-5 w-5 text-gray-400" />
        )}
        <Input
          type={isPassword ? (showPassword ? 'text' : 'password') : type}
          placeholder={placeholder}
          autoComplete={isPassword ? 'new-password' : 'off'}
          data-form-type="other"
          data-lpignore="true"
          {...field}
          className={cn(baseInputClasses, "h-12 text-base", className)}
        />
        {isPassword && (
          <button
            type="button"
            onClick={() => setShowPassword((v) => !v)}
            className="absolute right-3 text-gray-400 hover:text-gray-100 transition-colors"
            tabIndex={-1}
            aria-label={showPassword ? "Masquer le mot de passe" : "Afficher le mot de passe"}
          >
            {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
          </button>
        )}
      </div>
    );
  };

  return (
    <FormField
      control={form.control}
      name={name}
      render={({ field }) => (
        <FormItem>
          <FormControl>
            {renderInput(field)}
          </FormControl>
          
          {/* AJOUT : Affichage conditionnel de la description stylisée */}
          {description && (
            <FormDescription className={`text-xs text-gray-400 italic mt-1`}>
              {description}
            </FormDescription>
          )}

          <FormMessage className={`text-red-400 text-sm`} /> 
        </FormItem>
      )}
    />
  );
}

TorFormField.UserIcon = User;
TorFormField.LockIcon = Lock;
TorFormField.MailIcon = Mail;

export default TorFormField;