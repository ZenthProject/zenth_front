import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import fr from "@/locales/fr.json";
import en from "@/locales/en.json";
import de from "@/locales/de.json";
import es from "@/locales/es.json";
import pt from "@/locales/pt.json";
import ru from "@/locales/ru.json";
import zh from "@/locales/zh.json";
import ja from "@/locales/ja.json";
import hi from "@/locales/hi.json";
import it from "@/locales/it.json";

i18n.use(initReactI18next).init({
  resources: {
    fr: { translation: fr },
    en: { translation: en },
    de: { translation: de },
    es: { translation: es },
    pt: { translation: pt },
    ru: { translation: ru },
    zh: { translation: zh },
    ja: { translation: ja },
    hi: { translation: hi },
    it: { translation: it },
  },
  lng: localStorage.getItem("zenth_language") || "en",
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
