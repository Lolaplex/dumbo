import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  ChatDetail,
  ChatSummary,
  Exchange,
  ProviderView,
  TrayAction,
} from "./types";

export const ipc = {
  hideOverlay: () => invoke<void>("hide_overlay_cmd"),
  setOverlayHeight: (height: number) => invoke<void>("set_overlay_height", { height }),
  openSettings: () => invoke<void>("open_settings"),
  hideTrayMenu: () => invoke<void>("hide_tray_menu"),
  resizeTrayMenu: (height: number) => invoke<void>("resize_tray_menu", { height }),
  trayAction: (action: TrayAction) => invoke<void>("tray_action", { action }),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  listProviders: () => invoke<ProviderView[]>("list_providers"),
  upsertProvider: (provider: Omit<ProviderView, "hasKey">) =>
    invoke<ProviderView>("upsert_provider", { provider }),
  deleteProvider: (id: string) => invoke<void>("delete_provider", { id }),
  setProviderKey: (id: string, key: string) => invoke<void>("set_provider_key", { id, key }),
  listModels: (providerId: string) => invoke<string[]>("list_models", { providerId }),
  getSettings: () => invoke<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) => invoke<AppSettings>("save_settings", { settings }),
  startChat: (args: {
    requestId: string;
    providerId: string;
    model: string;
    prompt: string;
    selection: string | null;
    clipboard: string | null;
    detailed: boolean;
    prior: { role: string; content: string }[];
    chatId: string | null;
    attachments?: import("./types").ChatAttachment[];
  }) => invoke<void>("start_chat", args),
  abortChat: (requestId: string) => invoke<void>("abort_chat", { requestId }),
  listChats: () => invoke<ChatSummary[]>("list_chats"),
  listExchanges: (limit?: number) => invoke<Exchange[]>("list_exchanges", { limit: limit ?? null }),
  getChat: (id: string) => invoke<ChatDetail>("get_chat", { id }),
  deleteChat: (id: string) => invoke<void>("delete_chat", { id }),
  clearHistory: () => invoke<void>("clear_history"),
  setTtsKey: (id: string, key: string) => invoke<void>("set_tts_key", { id, key }),
  getTtsKeyStatus: (id: string) => invoke<boolean>("get_tts_key_status", { id }),
  stopTts: () => invoke<void>("stop_tts_cmd"),
  getTtsState: () => invoke<import("./types").TtsLiveState>("get_tts_state"),
  testTts: (payload: import("./types").TtsTestPayload) => invoke<void>("test_tts", { payload }),
  getLocalTtsStatus: (url?: string) => invoke<import("./types").LocalTtsStatus>("get_local_tts_status", { url: url ?? null }),
};
