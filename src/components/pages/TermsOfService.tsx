import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { ShieldAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import i18n from "@/lib/i18n";

const LANGUAGES = [
  { code: "en", flag: "🇬🇧" },
  { code: "fr", flag: "🇫🇷" },
  { code: "de", flag: "🇩🇪" },
  { code: "es", flag: "🇪🇸" },
  { code: "pt", flag: "🇵🇹" },
  { code: "ru", flag: "🇷🇺" },
  { code: "zh", flag: "🇨🇳" },
  { code: "ja", flag: "🇯🇵" },
  { code: "hi", flag: "🇮🇳" },
  { code: "it", flag: "🇮🇹" },
];

interface TermsOfServiceProps {
  onAccept: () => void;
  onDecline: () => void;
}

export default function TermsOfService({ onAccept, onDecline }: TermsOfServiceProps) {
  const { t } = useTranslation();

  const handleLanguageChange = (code: string) => {
    i18n.changeLanguage(code);
    localStorage.setItem("zenth_language", code);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-end sm:items-center justify-center bg-background/95 backdrop-blur-sm">
      <div className="w-full max-w-2xl sm:mx-4 flex flex-col bg-card border border-border sm:rounded-xl shadow-2xl h-[92vh] sm:max-h-[90vh] rounded-t-2xl">

        <div className="flex items-center justify-between px-5 sm:px-8 pt-5 pb-3 shrink-0">
          <div className="flex items-center gap-3">
            <ShieldAlert className="w-5 h-5 sm:w-6 sm:h-6 text-accent-secondary shrink-0" />
            <h1 className="text-lg sm:text-xl font-bold text-card-foreground tracking-wide">
              {t("tos.title")}
            </h1>
          </div>
          <div className="flex items-center gap-1 flex-wrap justify-end max-w-[180px]">
            {LANGUAGES.map((lang) => (
              <button
                key={lang.code}
                onClick={() => handleLanguageChange(lang.code)}
                className={`text-base leading-none rounded px-1 py-1 transition-all ${
                  i18n.language === lang.code
                    ? "opacity-100 ring-1 ring-primary"
                    : "opacity-40 hover:opacity-80"
                }`}
                title={lang.code.toUpperCase()}
              >
                {lang.flag}
              </button>
            ))}
          </div>
        </div>

        {/* Boutons en haut sur mobile pour éviter de devoir scroller */}
        <div className="flex gap-3 px-5 sm:px-8 pb-3 sm:hidden shrink-0">
          <Button variant="outline" className="flex-1 h-12 text-base" onClick={onDecline}>
            {t("tos.decline")}
          </Button>
          <Button className="flex-1 h-12 text-base" onClick={onAccept}>
            {t("tos.accept")}
          </Button>
        </div>

        <ScrollArea className="flex-1 px-5 sm:px-8 overflow-y-auto">
          <div className="prose prose-sm prose-invert max-w-none pb-6 text-muted-foreground space-y-6 leading-relaxed">

            <section>
              <h2 className="text-base font-semibold text-card-foreground mb-2">{t("tos.section_what_we_are")}</h2>
              <p>{t("tos.what_we_are_p1")}</p>
            </section>

            <section>
              <h2 className="text-base font-semibold text-card-foreground mb-2">{t("tos.section_what_we_can")}</h2>
              <p className="font-medium text-card-foreground">{t("tos.what_we_can_nothing")}</p>
              <p>{t("tos.what_we_can_p1")}</p>
              <p>{t("tos.what_we_can_p2")}</p>
              <p>{t("tos.what_we_can_p3")}</p>
            </section>

            <section>
              <h2 className="text-base font-semibold text-card-foreground mb-2">{t("tos.section_what_you_can")}</h2>
              <p>{t("tos.what_you_can_p1")}</p>
              <p>{t("tos.what_you_can_p2")}</p>
              <p>{t("tos.what_you_can_p3")}</p>
            </section>

            <section>
              <h2 className="text-base font-semibold text-card-foreground mb-2">{t("tos.section_states")}</h2>
              <p>{t("tos.states_p1")}</p>
              <p>{t("tos.states_p2")}</p>
              <ul className="list-disc pl-5 space-y-2">
                <li>
                  <strong className="text-card-foreground">{t("tos.states_item1_strong")}</strong>{" "}
                  {t("tos.states_item1_body")}
                </li>
                <li>
                  <strong className="text-card-foreground">{t("tos.states_item2_strong")}</strong>{" "}
                  {t("tos.states_item2_body")}
                </li>
              </ul>
              <p>{t("tos.states_p3")}</p>
              <p>{t("tos.states_p4")}</p>
            </section>

            <section>
              <h2 className="text-base font-semibold text-card-foreground mb-2">{t("tos.section_close_account")}</h2>
              <p>{t("tos.close_account_p1")}</p>
            </section>

            <section>
              <h2 className="text-base font-semibold text-card-foreground mb-2">{t("tos.section_responsibility")}</h2>
              <p>{t("tos.responsibility_p1")}</p>
            </section>

          </div>
        </ScrollArea>

        {/* Boutons en bas sur desktop */}
        <div className="hidden sm:flex gap-3 px-8 py-6 border-t border-border shrink-0">
          <Button variant="outline" className="flex-1" onClick={onDecline}>
            {t("tos.decline")}
          </Button>
          <Button className="flex-1" onClick={onAccept}>
            {t("tos.accept")}
          </Button>
        </div>

      </div>
    </div>
  );
}
