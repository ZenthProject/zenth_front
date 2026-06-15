import { z } from "zod";
import { zodResolver } from "@hookform/resolvers/zod";
import { invoke } from "@tauri-apps/api/core";
import { useForm } from "react-hook-form";
import { useState } from "react";
import { CheckCircle, XCircle, Lock, Shield, Check, X } from 'lucide-react';
import { Form } from "@/components/ui/form";
import { TorButton, TorFormField } from "@/components/modules/tor";
import { useTranslation } from "react-i18next";
import { Link, useNavigate } from "react-router-dom";

function PasswordRules({ password, t }: { password: string; t: (k: string) => string }) {
  const rules = [
    { key: t("register.validation.password_min").split("\n")[0],   ok: password.length >= 20 },
    { key: t("register.validation.password_uppercase"),             ok: (password.match(/[A-Z]/g) ?? []).length >= 3 },
    { key: t("register.validation.password_lowercase"),             ok: (password.match(/[a-z]/g) ?? []).length >= 3 },
    { key: t("register.validation.password_digits"),                ok: (password.match(/[0-9]/g) ?? []).length >= 3 },
    { key: t("register.validation.password_special"),               ok: (password.match(/[^a-zA-Z0-9]/g) ?? []).length >= 3 },
  ];

  if (!password) return null;

  return (
    <div className="grid grid-cols-1 gap-1 px-1">
      {rules.map((r) => (
        <div key={r.key} className="flex items-start gap-2">
          {r.ok
            ? <Check className="w-3.5 h-3.5 text-success shrink-0 mt-0.5" />
            : <X    className="w-3.5 h-3.5 text-destructive shrink-0 mt-0.5" />
          }
          <span className={`text-xs leading-tight ${r.ok ? "text-success" : "text-muted-foreground"}`}>
            {r.key}
          </span>
        </div>
      ))}
    </div>
  );
}

const noSimpleSeq = (str: string): boolean => {
  const sequences = [
    "abcdefghijklmnopqrstuvwxyz",
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    "0123456789",
    "!@#$%^&*()-_=+[]{}|;:',.<>/?`~"
  ];
  const length = 4;
  for (const seq of sequences) {
    for (let i = 0; i <= seq.length - length; i++) {
      const forwardSeq = seq.slice(i, i + length);
      const backwardSeq = forwardSeq.split("").reverse().join("");
      if (str.includes(forwardSeq) || str.includes(backwardSeq)) {
        return false;
      }
    }
  }
  return true;
};

export default function Register() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [isLoading, setIsLoading] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [isSuccess, setIsSuccess] = useState<boolean | null>(null);

  const registerSchema = z
    .object({
      username: z.string().min(1, t("register.validation.username_required")),
      password: z
        .string()
        .min(20, t("register.validation.password_min"))
        .refine((val) => (val.match(/[A-Z]/g) || []).length >= 3, {
          message: t("register.validation.password_uppercase"),
        })
        .refine((val) => (val.match(/[a-z]/g) || []).length >= 3, {
          message: t("register.validation.password_lowercase"),
        })
        .refine((val) => (val.match(/[0-9]/g) || []).length >= 3, {
          message: t("register.validation.password_digits"),
        })
        .refine((val) => (val.match(/[^A-Za-z0-9]/g) || []).length >= 3, {
          message: t("register.validation.password_special"),
        })
        .refine(noSimpleSeq, {
          message: t("register.validation.password_sequence"),
        })
        .refine((val) => !/(.)\1\1/.test(val), {
          message: t("register.validation.password_repeat"),
        }),
      confirmPassword: z.string(),
    })
    .refine((data) => data.password === data.confirmPassword, {
      message: t("register.validation.password_mismatch"),
      path: ["confirmPassword"],
    });

  type RegisterFormValues = z.infer<typeof registerSchema>;

  const form = useForm<RegisterFormValues>({
    resolver: zodResolver(registerSchema),
    defaultValues: {
      username: "",
      password: "",
      confirmPassword: "",
    },
  });

  const [usernameTaken, setUsernameTaken] = useState(false);

  const onSubmit = async (data: RegisterFormValues) => {
    setIsLoading(true);
    setStatusMessage(null);
    setIsSuccess(null);
    setUsernameTaken(false);

    try {
      const response = await invoke<string>("register", {
        username: data.username,
        password: data.password,
      });

      if (response.startsWith("Erreur")) {
        setIsSuccess(false);
        setStatusMessage(response);
      } else {
        setIsSuccess(true);
        setStatusMessage(t("register.success"));
        form.reset();
        setTimeout(() => navigate('/login'), 2000);
      }
    } catch (error) {
      console.error("Catch error:", error);
      setIsSuccess(false);
      const errorMsg = error instanceof Error ? error.message : String(error);
      if (errorMsg.toLowerCase().includes("already taken") || errorMsg.toLowerCase().includes("already exists")) {
        setUsernameTaken(true);
      } else {
        setStatusMessage(errorMsg);
      }
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="flex min-h-screen items-center justify-center bg-background py-8">
      <div className="w-[340px] sm:w-[400px] space-y-6">

        <div className="text-center space-y-1.5">
          <h1 className="text-2xl font-bold text-foreground">
            {t("register.title")}
          </h1>
          <p className="text-sm text-muted-foreground">
            {t("register.subtitle")}
          </p>
        </div>

        <Form {...form}>
          <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-5" autoComplete="off">

            <div className="flex items-start gap-2 text-sm text-muted-foreground border-l-2 border-primary pl-3">
              <Shield className="w-4 h-4 shrink-0 mt-0.5 text-primary" />
              <span>{t("register.e2e_notice_body")}</span>
            </div>

            <TorFormField
              form={form}
              name="username"
              placeholder={t("register.username_placeholder")}
              Icon={TorFormField.UserIcon}
            />

            <TorFormField
              form={form}
              name="password"
              placeholder={t("register.password_placeholder")}
              type="password"
              Icon={TorFormField.LockIcon}
            />

            <PasswordRules password={form.watch("password")} t={t} />

            <TorFormField
              form={form}
              name="confirmPassword"
              placeholder={t("register.confirm_placeholder")}
              type="password"
              Icon={TorFormField.LockIcon}
            />

            {usernameTaken && (
              <div className="border-l-2 border-amber-500 pl-3 space-y-1">
                <div className="flex items-center gap-2 text-sm font-medium text-amber-500">
                  <XCircle className="w-4 h-4 shrink-0" />
                  {t("register.username_taken")}
                </div>
                <p className="text-xs text-muted-foreground">
                  {t("register.username_taken_hint")}
                </p>
                <Link to="/login" className="text-xs text-primary underline hover:text-accent-secondary transition-colors">
                  {t("register.go_to_login")}
                </Link>
              </div>
            )}

            {statusMessage && !usernameTaken && (
              <div className={`flex items-center gap-2 p-2 text-sm rounded-md ${
                isSuccess
                  ? 'bg-success/10 text-success border border-success/30'
                  : 'bg-destructive/10 text-destructive border border-destructive/30'
              }`}>
                {isSuccess ? <CheckCircle className="w-4 h-4 shrink-0" /> : <XCircle className="w-4 h-4 shrink-0" />}
                {statusMessage}
              </div>
            )}

            <TorButton
              type="submit"
              isLoading={isLoading}
              loadingText={t("register.submitting")}
              Icon={Lock}
            >
              {t("register.submit")}
            </TorButton>
          </form>
        </Form>

        <p className="text-center text-sm text-muted-foreground">
          {t("register.has_account")}{" "}
          <Link to="/login" className="text-primary hover:text-accent-secondary font-medium underline transition-colors">
            {t("register.login_link")}
          </Link>
        </p>

      </div>
    </div>
  );
}
