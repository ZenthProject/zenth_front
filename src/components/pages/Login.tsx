import { z } from "zod";
import { zodResolver } from "@hookform/resolvers/zod";
import { invoke } from "@tauri-apps/api/core";
import { useForm } from "react-hook-form";
import { useState } from "react";
import { Lock } from 'lucide-react';
import { Form } from "@/components/ui/form";
import { Link, useNavigate } from "react-router-dom";
import { TorButton, TorFormField } from "@/components/modules/tor";
import { useAuth } from "@/hooks/use-auth";
import { useTranslation } from "react-i18next";
import TermsOfService from "@/components/pages/TermsOfService";
import { useUpdate } from "@/contexts/UpdateContext";
import UpdateBanner from "@/components/modules/UpdateBanner";

const TOS_KEY = (username: string) => `zenth_tos_accepted_${username}`;

export default function Login() {
  const { t } = useTranslation();
  const [isLoading, setIsLoading] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [isSuccess, setIsSuccess] = useState<boolean | null>(null);
  const [pendingUser, setPendingUser] = useState<{ username: string; sessionToken: string; password: string } | null>(null);
  const navigate = useNavigate();
  const { login: authLogin } = useAuth();
  const { setOutdated } = useUpdate();

  const loginSchema = z.object({
    username: z.string().min(1, t("login.validation.username_required")),
    password: z.string().min(1, t("login.validation.password_required")),
  });

  type LoginFormValues = z.infer<typeof loginSchema>;

  const rememberedUsername = localStorage.getItem('zenth_remember_username') ?? '';

  const form = useForm<LoginFormValues>({
    resolver: zodResolver(loginSchema),
    defaultValues: { username: rememberedUsername, password: "" },
  });

  const onSubmit = async (data: LoginFormValues) => {
    setIsLoading(true);
    setStatusMessage(null);
    setIsSuccess(null);

    try {
      const sessionToken = await invoke<string>("login", {
        username: data.username,
        password: data.password,
      });

      if (localStorage.getItem(TOS_KEY(data.username)) === 'true') {
        authLogin(data.username, sessionToken, data.password);
        setIsSuccess(true);
        setStatusMessage(t("login.success"));
        form.reset();
        setTimeout(() => navigate('/chat'), 1000);
      } else {
        setPendingUser({ username: data.username, sessionToken, password: data.password });
      }
    } catch (error) {
      setIsSuccess(false);
      const errMsg = error instanceof Error ? error.message : String(error);

      if (errMsg.includes("VERSION_OUTDATED:")) {
        const minVersion = errMsg.split("VERSION_OUTDATED:")[1]?.trim() ?? "";
        setOutdated(minVersion);
        setStatusMessage(t("login.version_outdated"));
        return;
      }

      if (errMsg.includes("SERVER_UNAVAILABLE")) {
        setStatusMessage(t("login.server_unavailable"));
        return;
      }

      // Le compteur d'échecs et le wipe sont gérés côté Rust (configure_wipe / login.rs)
      setStatusMessage(errMsg);
    } finally {
      setIsLoading(false);
    }
  };

  const handleTosAccept = () => {
    if (!pendingUser) return;
    localStorage.setItem(TOS_KEY(pendingUser.username), 'true');
    authLogin(pendingUser.username, pendingUser.sessionToken, pendingUser.password);
    setPendingUser(null);
    setTimeout(() => navigate('/chat'), 0);
  };

  const handleTosDecline = async () => {
    try {
      await invoke('logout', { sessionToken: pendingUser?.sessionToken });
    } catch {}
    setPendingUser(null);
    setStatusMessage(t("login.tos_required"));
    setIsSuccess(false);
  };

  return (
    <>
      {pendingUser && (
        <TermsOfService onAccept={handleTosAccept} onDecline={handleTosDecline} />
      )}
      <div className="fixed top-0 left-0 right-0 z-50">
        <UpdateBanner />
      </div>
      <div className="flex min-h-screen items-center justify-center bg-background">
        <div className="w-[340px] sm:w-[380px] space-y-8">

          <div className="text-center space-y-1.5">
            <h1 className="text-2xl font-bold text-foreground">
              {t("login.title")}
            </h1>
            <p className="text-sm text-muted-foreground">
              {t("login.subtitle")}
            </p>
          </div>

          <Form {...form}>
            <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-5" autoComplete="off">
              <TorFormField
                form={form}
                name="username"
                placeholder={t("login.username_placeholder")}
                Icon={TorFormField.UserIcon}
              />
              <TorFormField
                form={form}
                name="password"
                placeholder={t("login.password_placeholder")}
                type="password"
                Icon={TorFormField.LockIcon}
                description={t("login.password_hint")}
              />

              {statusMessage && (
                <p className={`text-xs text-center ${isSuccess ? 'text-muted-foreground' : 'text-destructive'}`}>
                  {statusMessage}
                </p>
              )}

              <TorButton
                type="submit"
                isLoading={isLoading}
                loadingText={t("login.submitting")}
                Icon={Lock}
              >
                {t("login.submit")}
              </TorButton>
            </form>
          </Form>

          <p className="text-center text-sm text-muted-foreground">
            {t("login.no_account")}{" "}
            <Link to="/keygen" className="text-primary hover:text-accent-secondary font-medium underline transition-colors">
              {t("login.register_link")}
            </Link>
          </p>

        </div>
      </div>
    </>
  );
}
