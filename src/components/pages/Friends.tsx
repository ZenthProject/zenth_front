import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Input } from "@/components/ui/input";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Key, UserPlus, Copy, Check, AlertCircle, Loader2, QrCode, X, Users, Clock, UserCheck, UserX, Trash2, RefreshCw, Wifi, WifiOff, Ban, ShieldOff, Camera, ShieldCheck, Pencil } from 'lucide-react';
import { FriendService } from '@/services/friendService';
import type { PendingRequest, FriendInfo } from '@/types/friends';
import { useAuth } from '@/hooks/use-auth';
import { useWebSocket } from '@/contexts/WebSocketContext';
import { notifyFriendRequest, notifyFriendAccepted } from '@/services/notificationService';
import { useTranslation } from 'react-i18next';
import { QRScanner } from '@/components/modules/QRScanner';

const isAndroid = /android/i.test(navigator.userAgent);

export default function AddFriendModule() {
  const { t } = useTranslation();
  const { username, sessionToken, isAuthenticated } = useAuth();
  const { isConnected, onFriendRequest, onFriendResponse } = useWebSocket();

  const [publicKey, setPublicKey] = useState('');
  const [nickname, setNickname] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [isCopied, setIsCopied] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [isSuccess, setIsSuccess] = useState<boolean | null>(null);
  const [showQRCode, setShowQRCode] = useState(false);
  const [showScanner, setShowScanner] = useState(false);
  const qrCodeRef = useRef<HTMLDivElement>(null);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const qrCodeInstance = useRef<any>(null);

  const [myPublicKey, setMyPublicKey] = useState<string>('');
  const [myAvatar, setMyAvatar] = useState<string | null>(null);
  const avatarInputRef = useRef<HTMLInputElement>(null);
  const friendAvatarInputRef = useRef<HTMLInputElement>(null);
  const [editingFriendId, setEditingFriendId] = useState<number | null>(null);
  const [renamingFriendId, setRenamingFriendId] = useState<number | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const [pendingRequests, setPendingRequests] = useState<PendingRequest[]>([]);
  const [friends, setFriends] = useState<FriendInfo[]>([]);
  const [blockedFriends, setBlockedFriends] = useState<FriendInfo[]>([]);
  const [loadingPending, setLoadingPending] = useState(false);
  const [loadingFriends, setLoadingFriends] = useState(false);
  const [isSyncing, setIsSyncing] = useState(false);
  const isSyncingRef = useRef(false);
  const [lastSyncResult, setLastSyncResult] = useState<string | null>(null);
  const [wsNotification, setWsNotification] = useState<string | null>(null);

  // Dialogs de confirmation blocage / suppression
  const [confirmBlock, setConfirmBlock] = useState<FriendInfo | null>(null);
  const [confirmRemove, setConfirmRemove] = useState<FriendInfo | null>(null);

  // State for accept dialog
  const [acceptingRequest, setAcceptingRequest] = useState<PendingRequest | null>(null);
  const [contactPseudo, setContactPseudo] = useState('');

  // State for verification modal
  const [verifyingFriend, setVerifyingFriend] = useState<FriendInfo | null>(null);
  const [fingerprint, setFingerprint] = useState<string | null>(null);
  const [fingerprintLoading, setFingerprintLoading] = useState(false);

  // Define load functions first (before syncFromServer which depends on them)
  const loadMyPublicKey = useCallback(async () => {
    if (!sessionToken) return;
    try {
      const key = await FriendService.getMyPublicKey({ sessionToken });
      setMyPublicKey(key);
    } catch (error) {
      console.error('Failed to load public key:', error);
    }
  }, [sessionToken]);

  const handleAvatarChange = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file || !sessionToken) return;
    e.target.value = '';

    const img = new Image();
    img.onload = async () => {
      const MAX = 256;
      const scale = Math.min(MAX / img.width, MAX / img.height, 1);
      const canvas = document.createElement('canvas');
      canvas.width = Math.round(img.width * scale);
      canvas.height = Math.round(img.height * scale);
      canvas.getContext('2d')!.drawImage(img, 0, 0, canvas.width, canvas.height);
      const b64 = canvas.toDataURL('image/jpeg', 0.85).split(',')[1];
      try {
        await FriendService.setMyAvatar({ sessionToken, avatarB64: b64 });
        setMyAvatar(b64);
      } catch (err) {
        console.error('Failed to save avatar:', err);
      }
    };
    img.src = URL.createObjectURL(file);
  }, [sessionToken]);

  const handleFriendAvatarChange = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file || !sessionToken || editingFriendId === null) return;
    e.target.value = '';

    const img = new Image();
    img.onload = async () => {
      const MAX = 256;
      const scale = Math.min(MAX / img.width, MAX / img.height, 1);
      const canvas = document.createElement('canvas');
      canvas.width = Math.round(img.width * scale);
      canvas.height = Math.round(img.height * scale);
      canvas.getContext('2d')!.drawImage(img, 0, 0, canvas.width, canvas.height);
      const b64 = canvas.toDataURL('image/jpeg', 0.85).split(',')[1];
      try {
        await FriendService.setFriendAvatar({ sessionToken, friendId: editingFriendId, avatarB64: b64 });
        setFriends(prev => prev.map(f => f.id === editingFriendId ? { ...f, avatar: b64 } : f));
      } catch (err) {
        console.error('Failed to save friend avatar:', err);
      }
      setEditingFriendId(null);
    };
    img.src = URL.createObjectURL(file);
  }, [sessionToken, editingFriendId]);

  const loadPendingRequests = useCallback(async () => {
    if (!sessionToken) return;
    setLoadingPending(true);
    try {
      const requests = await FriendService.listPendingRequests({ sessionToken });
      setPendingRequests(requests);
    } catch (error) {
      console.error('Failed to load pending requests:', error);
    } finally {
      setLoadingPending(false);
    }
  }, [sessionToken]);

  const loadFriends = useCallback(async () => {
    if (!sessionToken) return;
    setLoadingFriends(true);
    try {
      const [friendsList, blockedList] = await Promise.all([
        FriendService.listFriends({ sessionToken }),
        FriendService.listBlockedFriends({ sessionToken }),
      ]);
      setFriends(friendsList);
      setBlockedFriends(blockedList);
    } catch (error) {
      console.error('Failed to load friends:', error);
    } finally {
      setLoadingFriends(false);
    }
  }, [sessionToken]);

  const startRename = (friend: FriendInfo) => {
    setRenamingFriendId(friend.id);
    setRenameValue(friend.pseudo);
  };

  const confirmRename = async (friendId: number) => {
    const trimmed = renameValue.trim();
    if (!trimmed || !sessionToken) { setRenamingFriendId(null); return; }
    try {
      await FriendService.renameFriend(sessionToken, friendId, trimmed);
      setFriends(prev => prev.map(f => f.id === friendId ? { ...f, pseudo: trimmed } : f));
    } catch (e) {
      console.error('Failed to rename friend:', e);
    } finally {
      setRenamingFriendId(null);
    }
  };

  const syncFromServer = useCallback(async () => {
    if (!sessionToken || isSyncingRef.current) return;
    isSyncingRef.current = true;
    setIsSyncing(true);
    setLastSyncResult(null);

    try {
      // Relay pull + server sync en parallèle
      // syncAcceptedContacts couvre le cas : ils ont envoyé la demande, on a accepté
      const [requestsResult, responsesResult, acceptedResult, relayCount] = await Promise.all([
        FriendService.syncFriendRequests({ sessionToken }),
        FriendService.syncFriendResponses({ sessionToken }),
        FriendService.syncAcceptedContacts({ sessionToken }).catch(() => ({ new_incoming: 0, new_accepted: 0, errors: [] })),
        invoke<number>("relay_pull_messages", { sessionToken }).catch(() => 0),
      ]);

      const totalNewIncoming = requestsResult.new_incoming;
      const totalNewAccepted = responsesResult.new_accepted + acceptedResult.new_accepted;

      // Si le relay a rapporté de nouveaux événements, recharger immédiatement
      if (relayCount > 0) {
        loadPendingRequests();
        loadFriends();
      }

      if (totalNewIncoming > 0 || totalNewAccepted > 0) {
        const messages = [];
        if (totalNewIncoming > 0) {
          const autoAccept = localStorage.getItem("zenth_auto_accept_friend_requests") === "true";
          if (autoAccept) {
            // Auto-accept: fetch all pending and accept them silently
            try {
              const pending = await FriendService.listPendingRequests({ sessionToken: sessionToken! });
              await Promise.all(
                pending.map((req) =>
                  FriendService.acceptFriendRequest({
                    sessionToken: sessionToken!,
                    requesterHash: req.remote_username_hash,
                    pseudo: req.remote_pseudo || undefined,
                  }).catch(() => {/* ignore individual errors */})
                )
              );
              messages.push(t("friends.auto_accepted", { count: totalNewIncoming }));
            } catch {
              messages.push(t("friends.new_requests", { count: totalNewIncoming }));
            }
          } else {
            messages.push(t("friends.new_requests", { count: totalNewIncoming }));
            notifyFriendRequest(t("friends.new_friend_request"));
          }
        }
        if (totalNewAccepted > 0) {
          messages.push(t("friends.new_accepted", { count: totalNewAccepted }));
          notifyFriendAccepted(
            totalNewAccepted === 1
              ? t("friends.new_accepted", { count: 1 })
              : t("friends.new_accepted", { count: totalNewAccepted })
          );
        }
        setLastSyncResult(messages.join(', '));
        loadPendingRequests();
        loadFriends(); // Reload friends list to show newly accepted friends
      } else if (requestsResult.errors.length > 0 || responsesResult.errors.length > 0) {
        console.warn('Sync errors:', [...requestsResult.errors, ...responsesResult.errors]);
        setLastSyncResult(t("friends.sync_partial"));
      } else {
        setLastSyncResult(t("friends.sync_up_to_date"));
      }

      setTimeout(() => setLastSyncResult(null), 3000);
    } catch (error) {
      console.error('Sync failed:', error);
      setLastSyncResult(t("friends.sync_error"));
      setTimeout(() => setLastSyncResult(null), 3000);
    } finally {
      isSyncingRef.current = false;
      setIsSyncing(false);
    }
  }, [sessionToken, loadPendingRequests, loadFriends]);

  // Load my avatar on mount
  useEffect(() => {
    if (isAuthenticated && sessionToken) {
      FriendService.getMyAvatar({ sessionToken })
        .then(a => setMyAvatar(a))
        .catch(console.error);
    }
  }, [isAuthenticated, sessionToken]);

  // Initial load : affiche immédiatement depuis le cache, sync en arrière-plan
  useEffect(() => {
    if (isAuthenticated && sessionToken) {
      loadMyPublicKey();
      loadPendingRequests();
      loadFriends();
      syncFromServer();
    }
  }, [isAuthenticated, sessionToken, loadMyPublicKey, loadPendingRequests, loadFriends, syncFromServer]);

  // Periodic sync (reduced when WebSocket is connected)
  useEffect(() => {
    if (!isAuthenticated || !sessionToken) return;

    // Reduce polling interval when WebSocket is connected (use as fallback only)
    const interval = setInterval(() => {
      syncFromServer();
    }, isConnected ? 60000 : 30000); // 60s with WS, 30s without

    return () => clearInterval(interval);
  }, [isAuthenticated, sessionToken, isConnected, syncFromServer]);

  useEffect(() => {
    const unsubscribe = onFriendRequest((_notification) => {

      setWsNotification(t("friends.new_friend_request"));
      setTimeout(() => setWsNotification(null), 3000);

      loadPendingRequests();
    });

    return unsubscribe;
  }, [onFriendRequest, loadPendingRequests]);

  useEffect(() => {
    const unsubscribe = onFriendResponse((_notification, accepted) => {

      if (accepted) {
        setWsNotification(t("friends.request_accepted"));
        loadFriends();
      } else {
        setWsNotification(t("friends.request_rejected"));
      }
      setTimeout(() => setWsNotification(null), 3000);

      loadPendingRequests();
    });

    return unsubscribe;
  }, [onFriendResponse, loadPendingRequests, loadFriends]);

  // Réagit aux mises à jour relay (demandes d'ami + contacts depuis un autre appareil)
  useEffect(() => {
    const handler = () => {
      loadPendingRequests();
      loadFriends();
    };
    window.addEventListener("relay:update", handler);
    return () => window.removeEventListener("relay:update", handler);
  }, [loadPendingRequests, loadFriends]);

  useEffect(() => {
    if (!showQRCode || !qrCodeRef.current || !myPublicKey) return;
    const el = qrCodeRef.current;
    (async () => {
      const { default: QRCodeStyling } = await import('qr-code-styling');
      if (!qrCodeInstance.current) {
        qrCodeInstance.current = new QRCodeStyling({
          width: 300,
          height: 300,
          data: myPublicKey,
          margin: 10,
          qrOptions: { typeNumber: 0, mode: 'Byte', errorCorrectionLevel: 'M' },
          imageOptions: { hideBackgroundDots: true, imageSize: 0.4, margin: 0 },
          dotsOptions: { color: '#6366f1', type: 'rounded' },
          backgroundOptions: { color: 'transparent' },
          cornersSquareOptions: { color: '#a855f7', type: 'extra-rounded' },
          cornersDotOptions: { color: '#a855f7', type: 'dot' },
        });
      } else {
        qrCodeInstance.current.update({ data: myPublicKey });
      }
      el.innerHTML = '';
      qrCodeInstance.current.append(el);
    })();
  }, [showQRCode, myPublicKey]);

  const handleCopyKey = async () => {
    try {
      await navigator.clipboard.writeText(myPublicKey);
      setIsCopied(true);
      setTimeout(() => setIsCopied(false), 2000);
    } catch (err) {
      console.error('Copy error:', err);
    }
  };

  const handleAddFriend = async () => {
    if (!publicKey.trim()) {
      setIsSuccess(false);
      setStatusMessage(t("friends.error_key_required"));
      return;
    }

    if (!sessionToken) {
      setIsSuccess(false);
      setStatusMessage(t("friends.error_not_connected"));
      return;
    }

    const hexRegex = /^[a-fA-F0-9]{64}$/;
    if (!hexRegex.test(publicKey)) {
      setIsSuccess(false);
      setStatusMessage(t("friends.error_key_format"));
      return;
    }

    setIsLoading(true);
    setStatusMessage(null);
    setIsSuccess(null);

    try {
      await FriendService.sendFriendRequest({
        sessionToken,
        targetHash: publicKey,
        targetPseudo: nickname || undefined,
      });

      setIsSuccess(true);
      setStatusMessage(nickname ? t("friends.request_sent", { name: nickname }) : t("friends.request_sent_no_name"));
      setPublicKey('');
      setNickname('');

      loadPendingRequests();

      setTimeout(() => {
        setStatusMessage(null);
        setIsSuccess(null);
      }, 3000);
    } catch (error) {
      setIsSuccess(false);
      // Tauri returns errors as strings, not Error objects
      const errorMessage = typeof error === 'string'
        ? error
        : error instanceof Error
          ? error.message
          : String(error);
      console.error('Send friend request error:', error);
      setStatusMessage(errorMessage);
    } finally {
      setIsLoading(false);
    }
  };

  // Open accept dialog for a request
  const openAcceptDialog = (request: PendingRequest) => {
    setAcceptingRequest(request);
    setContactPseudo(request.remote_pseudo || '');
  };

  // Close accept dialog
  const closeAcceptDialog = () => {
    setAcceptingRequest(null);
    setContactPseudo('');
  };

  // Confirm accept with custom pseudo (called from dialog)
  const confirmAcceptRequest = async () => {
    if (!sessionToken || !acceptingRequest) return;

    try {
      await FriendService.acceptFriendRequest({
        sessionToken,
        requesterHash: acceptingRequest.remote_username_hash,
        pseudo: contactPseudo.trim() || undefined,
      });

      closeAcceptDialog();
      loadPendingRequests();
      loadFriends();
      setIsSuccess(true);
      setStatusMessage(t("friends.friend_added"));
      setTimeout(() => setStatusMessage(null), 3000);
    } catch (error) {
      console.error('Failed to accept request:', error);
      setIsSuccess(false);
      const errorMessage = typeof error === 'string'
        ? error
        : error instanceof Error
          ? error.message
          : String(error);
      setStatusMessage(errorMessage);
    }
  };

  const handleRejectRequest = async (requesterHash: string) => {
    if (!sessionToken) return;

    try {
      await FriendService.rejectFriendRequest({
        sessionToken,
        requesterHash,
      });

      loadPendingRequests();
    } catch (error) {
      console.error('Failed to reject request:', error);
      setIsSuccess(false);
      setStatusMessage(error instanceof Error ? error.message : String(error));
    }
  };

  const handleRemoveFriend = async (friendId: number) => {
    if (!sessionToken) return;

    try {
      await FriendService.removeFriend({
        sessionToken,
        friendId,
      });

      loadFriends();
    } catch (error) {
      console.error('Failed to remove friend:', error);
    }
  };

  const handleBlockFriend = async (friendId: number) => {
    if (!sessionToken) return;
    try {
      await FriendService.blockFriend({ sessionToken, friendId });
      loadFriends();
    } catch (error) {
      console.error('Failed to block friend:', error);
    }
  };

  const openVerifyModal = async (friend: FriendInfo) => {
    setVerifyingFriend(friend);
    setFingerprint(null);
    setFingerprintLoading(true);
    try {
      const fp = await invoke<string>('get_friend_fingerprint', { sessionToken, friendId: friend.id });
      setFingerprint(fp);
    } catch (e) {
      setFingerprint(null);
    } finally {
      setFingerprintLoading(false);
    }
  };

  const confirmVerified = async () => {
    if (!sessionToken || !verifyingFriend) return;
    try {
      await invoke('mark_friend_verified', { sessionToken, friendId: verifyingFriend.id });
      setFriends(prev => prev.map(f => f.id === verifyingFriend.id ? { ...f, verified: true } : f));
      setVerifyingFriend(null);
    } catch (e) {
      console.error('mark_friend_verified failed:', e);
    }
  };

  const handleUnblockFriend = async (friendId: number) => {
    if (!sessionToken) return;
    try {
      await FriendService.unblockFriend({ sessionToken, friendId });
      loadFriends();
    } catch (error) {
      console.error('Failed to unblock friend:', error);
    }
  };

  const incomingRequests = pendingRequests.filter(r => r.direction === 'incoming');
  const outgoingRequests = pendingRequests.filter(r => r.direction === 'outgoing');

  if (!isAuthenticated) {
    return (
      <div className="h-full flex items-center justify-center">
        <p className="text-sm text-muted-foreground">{t("friends.not_authenticated")}</p>
      </div>
    );
  }

  const ConfirmDialog = ({ title, body, warning, onConfirm, onCancel, confirmLabel, confirmClass }: {
    title: string; body: string; warning?: string;
    onConfirm: () => void; onCancel: () => void;
    confirmLabel: string; confirmClass: string;
  }) => (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onClick={onCancel}>
      <div className="bg-card border border-border rounded-2xl p-5 mx-6 shadow-2xl max-w-sm w-full"
        onClick={e => e.stopPropagation()}>
        <p className="text-sm font-semibold text-foreground mb-1">{title}</p>
        <p className="text-xs text-muted-foreground mb-3">{body}</p>
        {warning && (
          <div className="flex items-start gap-2 bg-destructive/10 border border-destructive/20 rounded-xl px-3 py-2 mb-4">
            <span className="text-destructive text-xs mt-0.5">⚠</span>
            <p className="text-xs text-destructive">{warning}</p>
          </div>
        )}
        <div className="flex gap-2 justify-end">
          <button onClick={onCancel}
            className="px-3 py-1.5 text-xs rounded-lg bg-secondary text-foreground">
            {t("friends.confirm_cancel")}
          </button>
          <button onClick={onConfirm}
            className={`px-3 py-1.5 text-xs rounded-lg text-white ${confirmClass}`}>
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Dialog blocage */}
      {confirmBlock && (
        <ConfirmDialog
          title={t("friends.confirm_block_title", { name: confirmBlock.pseudo })}
          body={t("friends.confirm_block_body")}
          warning={t("friends.confirm_block_warning")}
          onCancel={() => setConfirmBlock(null)}
          onConfirm={() => { handleBlockFriend(confirmBlock.id); setConfirmBlock(null); }}
          confirmLabel={t("friends.confirm_block_btn")}
          confirmClass="bg-warning/80 hover:bg-warning"
        />
      )}
      {confirmRemove && (
        <ConfirmDialog
          title={t("friends.confirm_remove_title", { name: confirmRemove.pseudo })}
          body={t("friends.confirm_remove_body")}
          warning={t("friends.confirm_remove_warning")}
          onCancel={() => setConfirmRemove(null)}
          onConfirm={() => { handleRemoveFriend(confirmRemove.id); setConfirmRemove(null); }}
          confirmLabel={t("friends.confirm_remove_btn")}
          confirmClass="bg-destructive hover:bg-destructive/90"
        />
      )}

      {/* WS notification toast */}
      {wsNotification && (
        <div className="fixed top-4 right-4 z-50 animate-in slide-in-from-top-2 fade-in duration-300">
          <div className="bg-primary text-primary-foreground px-4 py-3 rounded-lg shadow-lg flex items-center gap-2 text-sm">
            <Wifi className="w-4 h-4" />
            {wsNotification}
          </div>
        </div>
      )}

      {/* Header */}
      <div className="flex items-center justify-between px-6 py-4 border-b border-border shrink-0">
        <div className="flex items-center gap-3">
          <h1 className="text-lg font-semibold text-foreground">{t("friends.title")}</h1>
          {friends.length > 0 && (
            <span className="text-xs text-muted-foreground">{friends.length}</span>
          )}
        </div>
        <div className="flex items-center gap-3">
          {lastSyncResult && (
            <span className="text-xs text-muted-foreground">{lastSyncResult}</span>
          )}
          {!isAndroid && (
            <span className={`text-xs px-2 py-0.5 rounded-full ${
              isConnected ? 'bg-success/15 text-success' : 'bg-muted text-muted-foreground'
            }`}>
              {isConnected ? <Wifi className="w-3 h-3 inline mr-1" /> : <WifiOff className="w-3 h-3 inline mr-1" />}
              {isConnected ? t("friends.status_realtime") : t("friends.status_polling")}
            </span>
          )}
          <button
            onClick={syncFromServer}
            disabled={isSyncing}
            className="p-1.5 rounded-lg hover:bg-muted transition-colors disabled:opacity-50"
            title={t("friends.sync")}
          >
            <RefreshCw className={`w-4 h-4 text-muted-foreground ${isSyncing ? 'animate-spin' : ''}`} />
          </button>
        </div>
      </div>

      {/* Body - two columns on desktop, stacked on mobile */}
      <div className="flex-1 flex flex-col sm:flex-row overflow-auto sm:overflow-hidden">

        {/* Left column: contacts + pending */}
        <div className="flex-1 sm:overflow-y-auto">

          {/* Incoming requests */}
          {loadingPending ? (
            <div className="flex justify-center py-6">
              <Loader2 className="w-5 h-5 animate-spin text-muted-foreground" />
            </div>
          ) : incomingRequests.length > 0 && (
            <div className="px-6 py-4 border-b border-border">
              <p className="text-xs text-muted-foreground uppercase tracking-wide mb-3">
                {t("friends.incoming_requests", { count: incomingRequests.length })}
              </p>
              <div className="space-y-1">
                {incomingRequests.map((request) => (
                  <div key={request.id} className="flex items-center gap-3 px-2 py-2.5 rounded-lg">
                    <div className="h-8 w-8 rounded-full bg-warning/15 flex items-center justify-center shrink-0">
                      <Users className="h-4 w-4 text-warning" />
                    </div>
                    <div className="flex-1 min-w-0">
                      <p className="text-sm text-foreground font-medium">
                        {request.remote_pseudo || request.remote_username_hash.slice(0, 8) + '...'}
                      </p>
                      {request.message && (
                        <p className="text-xs text-muted-foreground truncate italic">"{request.message}"</p>
                      )}
                    </div>
                    <div className="flex gap-1 shrink-0">
                      <button
                        onClick={() => openAcceptDialog(request)}
                        className="p-1.5 rounded-lg hover:bg-success/15 text-success transition-colors"
                        title={t("friends.accept")}
                      >
                        <UserCheck className="w-4 h-4" />
                      </button>
                      <button
                        onClick={() => handleRejectRequest(request.remote_username_hash)}
                        className="p-1.5 rounded-lg hover:bg-destructive/15 text-destructive transition-colors"
                        title={t("friends.request_rejected")}
                      >
                        <UserX className="w-4 h-4" />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Outgoing requests */}
          {outgoingRequests.length > 0 && (
            <div className="px-6 py-4 border-b border-border">
              <p className="text-xs text-muted-foreground uppercase tracking-wide mb-3">
                {t("friends.outgoing_requests", { count: outgoingRequests.length })}
              </p>
              <div className="space-y-1">
                {outgoingRequests.map((request) => (
                  <div key={request.id} className="flex items-center gap-3 px-2 py-2.5 rounded-lg">
                    <div className="h-8 w-8 rounded-full bg-muted flex items-center justify-center shrink-0">
                      <Clock className="h-4 w-4 text-muted-foreground" />
                    </div>
                    <p className="flex-1 text-sm text-foreground">
                      {request.remote_pseudo || request.remote_username_hash.slice(0, 8) + '...'}
                    </p>
                    <span className="text-xs text-muted-foreground shrink-0">{t("friends.pending")}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Hidden input for friend avatar */}
          <input
            ref={friendAvatarInputRef}
            type="file"
            accept="image/*"
            className="hidden"
            onChange={handleFriendAvatarChange}
          />

          {/* Friends list */}
          <div className="px-6 py-4">
            {loadingFriends ? (
              <div className="flex justify-center py-8">
                <Loader2 className="w-5 h-5 animate-spin text-muted-foreground" />
              </div>
            ) : friends.length > 0 ? (
              <>
                <p className="text-xs text-muted-foreground uppercase tracking-wide mb-3">{t("friends.friends_list")}</p>
                <div className="space-y-1">
                  {friends.map((friend) => (
                    <div
                      key={friend.id}
                      className="group flex items-center gap-3 px-3 py-2.5 rounded-lg hover:bg-muted/60 transition-colors"
                    >
                      <button
                        className="relative shrink-0"
                        onClick={() => { setEditingFriendId(friend.id); friendAvatarInputRef.current?.click(); }}
                        title={t("friends.change_photo")}
                      >
                        <Avatar className="h-8 w-8">
                          {friend.avatar && (
                            <AvatarImage src={`data:image/jpeg;base64,${friend.avatar}`} alt={friend.pseudo} />
                          )}
                          <AvatarFallback className="bg-gradient-to-br from-primary to-accent-secondary text-white text-sm font-medium">
                            {friend.pseudo.charAt(0).toUpperCase()}
                          </AvatarFallback>
                        </Avatar>
                        <div className="absolute inset-0 rounded-full bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                          <Camera className="w-3 h-3 text-white" />
                        </div>
                      </button>
                      <div className="flex-1 min-w-0">
                        {renamingFriendId === friend.id ? (
                          <Input
                            autoFocus
                            value={renameValue}
                            onChange={e => setRenameValue(e.target.value)}
                            onBlur={() => confirmRename(friend.id)}
                            onKeyDown={e => {
                              if (e.key === 'Enter') confirmRename(friend.id);
                              if (e.key === 'Escape') setRenamingFriendId(null);
                            }}
                            className="h-7 text-sm px-2 py-0"
                          />
                        ) : (
                          <p className="text-sm text-foreground">{friend.pseudo}</p>
                        )}
                        {friend.verified ? (
                          <p className="text-xs text-success flex items-center gap-1">
                            <ShieldCheck className="w-3 h-3" /> {t("friends.verified")}
                          </p>
                        ) : (
                          <p className="text-xs text-muted-foreground">{t("friends.not_verified")}</p>
                        )}
                      </div>
                      {/* Sur Android (pas de hover) : boutons toujours visibles */}
                      <button
                        onClick={() => startRename(friend)}
                        className={`p-1.5 rounded-lg hover:bg-muted text-muted-foreground transition-all ${isAndroid ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'}`}
                        title="Renommer"
                      >
                        <Pencil className="w-3.5 h-3.5" />
                      </button>
                      <button
                        onClick={() => openVerifyModal(friend)}
                        className={`p-1.5 rounded-lg transition-all ${friend.verified ? 'text-success' : 'text-warning'} ${isAndroid ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'} hover:bg-muted`}
                        title={t("friends.verify_identity")}
                      >
                        <ShieldCheck className="w-3.5 h-3.5" />
                      </button>
                      <button
                        onClick={() => setConfirmBlock(friend)}
                        className={`p-1.5 rounded-lg hover:bg-warning/15 text-warning transition-all ${isAndroid ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'}`}
                        title={t("friends.action_block")}
                      >
                        <Ban className="w-3.5 h-3.5" />
                      </button>
                      <button
                        onClick={() => setConfirmRemove(friend)}
                        className={`p-1.5 rounded-lg hover:bg-destructive/15 text-destructive transition-all ${isAndroid ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'}`}
                        title={t("friends.action_delete")}
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  ))}
                </div>
              </>
            ) : (
              <p className="text-sm text-muted-foreground py-4">{t("friends.no_contacts")}</p>
            )}
          </div>

          {/* Blocked */}
          {blockedFriends.length > 0 && (
            <div className="px-6 py-4 border-t border-border">
              <p className="text-xs text-muted-foreground uppercase tracking-wide mb-3">{t("friends.blocked", { count: blockedFriends.length })}</p>
              <div className="space-y-1">
                {blockedFriends.map((friend) => (
                  <div
                    key={friend.id}
                    className="group flex items-center gap-3 px-3 py-2.5 rounded-lg hover:bg-muted/40 transition-colors"
                  >
                    <Avatar className="h-8 w-8 shrink-0">
                      {friend.avatar && (
                        <AvatarImage src={`data:image/jpeg;base64,${friend.avatar}`} alt={friend.pseudo} />
                      )}
                      <AvatarFallback className="bg-muted text-muted-foreground text-sm font-medium">
                        {friend.pseudo.charAt(0).toUpperCase()}
                      </AvatarFallback>
                    </Avatar>
                    <p className="flex-1 text-sm text-muted-foreground">{friend.pseudo}</p>
                    <button
                      onClick={() => handleUnblockFriend(friend.id)}
                      className={`p-1.5 rounded-lg hover:bg-success/15 text-success transition-all ${isAndroid ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'}`}
                      title={t("friends.action_unblock")}
                    >
                      <ShieldOff className="w-3.5 h-3.5" />
                    </button>
                    <button
                      onClick={() => setConfirmRemove(friend)}
                      className={`p-1.5 rounded-lg hover:bg-destructive/15 text-destructive transition-all ${isAndroid ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'}`}
                      title={t("friends.action_delete")}
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Right column: my key + add friend */}
        <div className="sm:w-72 border-t sm:border-t-0 sm:border-l border-border sm:overflow-y-auto flex flex-col">

          {/* My avatar */}
          <div className="px-5 py-4 border-b border-border flex flex-col items-center gap-3">
            <input
              ref={avatarInputRef}
              type="file"
              accept="image/*"
              className="hidden"
              onChange={handleAvatarChange}
            />
            <button
              onClick={() => avatarInputRef.current?.click()}
              className="relative group"
              title={t("friends.change_avatar")}
            >
              <Avatar className="h-16 w-16 ring-2 ring-border">
                {myAvatar && (
                  <AvatarImage src={`data:image/jpeg;base64,${myAvatar}`} alt={username || ''} />
                )}
                <AvatarFallback className="bg-gradient-to-br from-primary to-accent-secondary text-primary-foreground text-xl font-semibold">
                  {username?.charAt(0).toUpperCase() || '?'}
                </AvatarFallback>
              </Avatar>
              <div className="absolute inset-0 rounded-full bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                <Camera className="w-5 h-5 text-white" />
              </div>
            </button>
            <p className="text-xs text-muted-foreground">{username}</p>
          </div>

          {/* My public key */}
          <div className="px-5 py-4 border-b border-border">
            <p className="text-xs text-muted-foreground uppercase tracking-wide mb-3 flex items-center gap-1.5">
              <Key className="w-3 h-3" />
              {t("friends.my_public_key")}
            </p>
            <div className="bg-muted rounded-lg p-3 mb-3">
              <code className="text-xs text-foreground break-all font-mono leading-relaxed">
                {myPublicKey || t("friends.loading")}
              </code>
            </div>
            <div className="flex gap-2">
              <button
                onClick={handleCopyKey}
                disabled={!myPublicKey}
                className="flex-1 flex items-center justify-center gap-1.5 py-2 rounded-lg bg-muted hover:bg-muted/80 text-sm text-foreground transition-colors disabled:opacity-50"
              >
                {isCopied ? <Check className="w-3.5 h-3.5 text-success" /> : <Copy className="w-3.5 h-3.5" />}
                {isCopied ? t("friends.copied") : t("friends.copy")}
              </button>
              <button
                onClick={() => setShowQRCode(true)}
                disabled={!myPublicKey}
                className="flex items-center justify-center px-3 py-2 rounded-lg bg-muted hover:bg-muted/80 text-foreground transition-colors disabled:opacity-50"
                title="QR Code"
              >
                <QrCode className="w-3.5 h-3.5" />
              </button>
              {isAndroid && <button
                onClick={async () => {
                  if (!myPublicKey) return;
                  const shareMessage = `Salut ! Voici ma clé publique Zenth :\n\n${myPublicKey}\n\nAjoute-moi sur Zenth !`;
                  // 1. Commande Rust JNI → vrai Android share sheet
                  try {
                    await invoke('share_text', { text: shareMessage });
                    return;
                  } catch { /* non Android ou erreur JNI */ }
                  // 2. Web Share API (desktop)
                  try {
                    if (navigator.share) {
                      await navigator.share({ title: 'Ma clé publique Zenth', text: shareMessage });
                      return;
                    }
                  } catch { /* annulé ou non supporté */ }
                  // 3. Fallback clipboard
                  handleCopyKey();
                }}
                disabled={!myPublicKey}
                className="flex items-center justify-center px-3 py-2 rounded-lg bg-primary/10 hover:bg-primary/20 text-primary transition-colors disabled:opacity-50"
                title="Partager via..."
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="18" cy="5" r="3"/><circle cx="6" cy="12" r="3"/><circle cx="18" cy="19" r="3"/><line x1="8.59" y1="13.51" x2="15.42" y2="17.49"/><line x1="15.41" y1="6.51" x2="8.59" y2="10.49"/></svg>
              </button>}
            </div>
          </div>

          {/* Add friend */}
          <div className="px-5 py-4 flex-1">
            <p className="text-xs text-muted-foreground uppercase tracking-wide mb-3 flex items-center gap-1.5">
              <UserPlus className="w-3 h-3" />
              {t("friends.add_contact")}
            </p>
            <div className="space-y-3">
              {/* Modal scanner */}
              {showScanner && (
                <div className="fixed inset-0 z-50 flex items-end sm:items-center justify-center bg-black/70 backdrop-blur-sm"
                  onClick={() => setShowScanner(false)}>
                  <div className="bg-card border border-border rounded-t-2xl sm:rounded-2xl p-5 w-full max-w-sm shadow-2xl"
                    onClick={e => e.stopPropagation()}>
                    <div className="flex items-center justify-between mb-4">
                      <p className="text-sm font-semibold text-foreground">{t("qr_scanner.title")}</p>
                      <button onClick={() => setShowScanner(false)} className="text-muted-foreground hover:text-foreground">
                        <X className="w-4 h-4" />
                      </button>
                    </div>
                    <QRScanner
                      pasteDescription={t("qr_scanner.paste_contact_desc")}
                      pastePlaceholder={t("qr_scanner.paste_contact_placeholder")}
                      onScan={(data) => {
                        const trimmed = data.trim();
                        const hexMatch = trimmed.match(/[0-9a-fA-F]{64}/);
                        if (hexMatch) {
                          setPublicKey(hexMatch[0]);
                          setShowScanner(false);
                        }
                      }}
                    />
                  </div>
                </div>
              )}
              <div className="flex gap-2">
                <Input
                  type="text"
                  placeholder={t("friends.public_key_placeholder")}
                  value={publicKey}
                  onChange={(e) => setPublicKey(e.target.value)}
                  className="font-mono text-xs bg-muted border-0 focus-visible:ring-1 flex-1"
                />
                <button
                  onClick={() => setShowScanner(true)}
                  className="shrink-0 p-2 rounded-lg bg-muted hover:bg-primary/20 text-muted-foreground hover:text-primary transition-colors"
                  title="Scanner un QR code"
                >
                  <QrCode className="w-5 h-5" />
                </button>
              </div>
              <Input
                type="text"
                placeholder={t("friends.pseudo_placeholder")}
                value={nickname}
                onChange={(e) => setNickname(e.target.value)}
                className="text-sm bg-muted border-0 focus-visible:ring-1"
              />
              {statusMessage && (
                <p className={`text-xs px-2 py-1.5 rounded ${
                  isSuccess ? 'text-success bg-success/10' : 'text-destructive bg-destructive/10'
                }`}>
                  {statusMessage}
                </p>
              )}
              <button
                onClick={handleAddFriend}
                disabled={isLoading}
                className="w-full flex items-center justify-center gap-2 py-2.5 rounded-lg bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors disabled:opacity-50"
              >
                {isLoading ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <UserPlus className="w-4 h-4" />
                )}
                {isLoading ? t("friends.sending") : t("friends.send_request")}
              </button>
              <p className="text-xs text-muted-foreground flex items-start gap-1.5">
                <AlertCircle className="w-3 h-3 mt-0.5 shrink-0" />
                {t("friends.key_format_hint")}
              </p>
            </div>
          </div>
        </div>
      </div>

      {/* QR Code Modal */}
      {showQRCode && (
        <div
          className="fixed inset-0 bg-black/75 flex items-center justify-center z-50 p-6"
          onClick={() => setShowQRCode(false)}
        >
          <div
            className="bg-card border border-border rounded-xl shadow-2xl w-full max-w-sm"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between px-5 py-4 border-b border-border">
              <span className="text-sm font-medium text-foreground flex items-center gap-2">
                <QrCode className="w-4 h-4 text-primary" />
                QR Code
              </span>
              <button
                onClick={() => setShowQRCode(false)}
                className="p-1.5 hover:bg-muted rounded-lg transition-colors"
              >
                <X className="w-4 h-4 text-muted-foreground" />
              </button>
            </div>
            <div className="p-5 space-y-4">
              <div className="flex justify-center p-4 bg-muted rounded-lg">
                <div ref={qrCodeRef}></div>
              </div>
              <div className="bg-muted rounded-lg p-3">
                <code className="text-xs text-muted-foreground break-all font-mono block text-center">
                  {myPublicKey}
                </code>
              </div>
              <button
                onClick={() => setShowQRCode(false)}
                className="w-full py-2.5 rounded-lg bg-muted hover:bg-muted/80 text-sm text-foreground transition-colors"
              >
                {t("common.close")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Verify Identity Modal */}
      {verifyingFriend && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-card border border-border rounded-xl p-6 w-full max-w-md mx-4 shadow-2xl">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-base font-semibold text-foreground flex items-center gap-2">
                <ShieldCheck className="w-4 h-4 text-success" />
                {t("friends.verify_identity")}
              </h3>
              <button onClick={() => setVerifyingFriend(null)} className="p-1.5 hover:bg-muted rounded-lg transition-colors">
                <X className="w-4 h-4 text-muted-foreground" />
              </button>
            </div>

            <p className="text-sm text-muted-foreground mb-4">
              {t("friends.verify_instructions", { name: verifyingFriend.pseudo })}
            </p>

            <div className="bg-muted rounded-xl p-4 mb-4">
              {fingerprintLoading ? (
                <div className="flex justify-center py-2">
                  <Loader2 className="w-5 h-5 animate-spin text-muted-foreground" />
                </div>
              ) : fingerprint ? (
                <code className="text-sm font-mono text-foreground tracking-widest leading-relaxed block text-center break-all">
                  {fingerprint.split(' ').map((group, i) => (
                    <span key={i} className="inline-block mx-1 my-0.5 px-2 py-1 bg-background rounded text-xs">
                      {group}
                    </span>
                  ))}
                </code>
              ) : (
                <p className="text-xs text-destructive text-center">{t("friends.fingerprint_error")}</p>
              )}
            </div>

            <p className="text-xs text-muted-foreground mb-5 flex items-start gap-1.5">
              <AlertCircle className="w-3 h-3 mt-0.5 shrink-0" />
              {t("friends.verify_hint")}
            </p>

            <div className="flex gap-3">
              <button
                onClick={() => setVerifyingFriend(null)}
                className="flex-1 py-2.5 rounded-lg border border-border hover:bg-muted text-sm text-foreground transition-colors"
              >
                {t("common.cancel")}
              </button>
              <button
                onClick={confirmVerified}
                disabled={!fingerprint || verifyingFriend.verified}
                className="flex-1 py-2.5 rounded-lg bg-success hover:bg-success/90 text-success-foreground text-sm font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
              >
                <ShieldCheck className="w-4 h-4" />
                {verifyingFriend.verified ? t("friends.already_verified") : t("friends.confirm_verified")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Accept Request Dialog */}
      {acceptingRequest && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-card border border-border rounded-xl p-6 w-full max-w-md mx-4 shadow-2xl">
            <h3 className="text-base font-semibold text-foreground mb-4 flex items-center gap-2">
              <UserCheck className="w-4 h-4 text-success" />
              {t("friends.accept_request")}
            </h3>
            <div className="space-y-4">
              <p className="text-sm text-muted-foreground">
                {t("friends.from")} <code className="text-xs font-mono">{acceptingRequest.remote_username_hash.slice(0, 16)}...</code>
              </p>
              {acceptingRequest.message && (
                <p className="text-sm text-muted-foreground italic">"{acceptingRequest.message}"</p>
              )}
              <div>
                <label className="block text-sm text-foreground mb-1.5">{t("friends.contact_name")}</label>
                <Input
                  type="text"
                  placeholder={t("friends.default_name_placeholder")}
                  value={contactPseudo}
                  onChange={(e) => setContactPseudo(e.target.value)}
                  className="w-full bg-muted border-0 focus-visible:ring-1"
                  autoFocus
                />
              </div>
              <div className="flex gap-3 pt-1">
                <button
                  onClick={closeAcceptDialog}
                  className="flex-1 py-2.5 rounded-lg border border-border hover:bg-muted text-sm text-foreground transition-colors"
                >
                  {t("common.cancel")}
                </button>
                <button
                  onClick={confirmAcceptRequest}
                  className="flex-1 py-2.5 rounded-lg bg-success hover:bg-success/90 text-success-foreground text-sm font-medium transition-colors flex items-center justify-center gap-2"
                >
                  <UserCheck className="w-4 h-4" />
                  {t("friends.accept")}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
