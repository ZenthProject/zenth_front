import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

function isNotificationsEnabled(): boolean {
  return localStorage.getItem("zenth_notifications_enabled") !== "false";
}

function isSoundEnabled(): boolean {
  return localStorage.getItem("zenth_notification_sound") !== "false";
}

const isAndroid = navigator.userAgent.toLowerCase().includes("android");

// Résout l'icône selon la plateforme :
// - Android : nom du drawable (monochrome, rendu blanc par le système)
// - Desktop  : chemin absolu vers l'icône de l'app via resolveResource
let _iconCache: string | undefined;
async function getIcon(): Promise<string | undefined> {
  if (_iconCache !== undefined) return _iconCache;
  try {
    if (isAndroid) {
      _iconCache = "ic_notification";
    } else {
      const { resolveResource } = await import("@tauri-apps/api/path");
      _iconCache = await resolveResource("icons/128x128.png");
    }
  } catch {
    _iconCache = "";
  }
  return _iconCache || undefined;
}

async function ensurePermission(): Promise<boolean> {
  let granted = await isPermissionGranted();
  if (!granted) {
    const permission = await requestPermission();
    granted = permission === "granted";
  }
  return granted;
}

export async function notifyNewMessage(senderName: string): Promise<void> {
  if (!isNotificationsEnabled()) return;
  const granted = await ensurePermission();
  if (!granted) return;

  sendNotification({
    title: `Message de ${senderName}`,
    body: "Nouveau message chiffré",
    icon: await getIcon(),
    sound: isSoundEnabled() ? "default" : undefined,
  });
}

export async function notifyFriendRequest(senderName: string): Promise<void> {
  if (!isNotificationsEnabled()) return;
  const granted = await ensurePermission();
  if (!granted) return;

  sendNotification({
    title: "Demande d'ami",
    body: `${senderName} souhaite vous ajouter`,
    icon: await getIcon(),
    sound: isSoundEnabled() ? "default" : undefined,
  });
}

export async function notifyFriendAccepted(body: string): Promise<void> {
  if (!isNotificationsEnabled()) return;
  const granted = await ensurePermission();
  if (!granted) return;

  sendNotification({
    title: "Demande acceptée",
    body,
    icon: await getIcon(),
    sound: isSoundEnabled() ? "default" : undefined,
  });
}
