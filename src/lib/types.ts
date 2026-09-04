export type ProviderKind = "gemini" | "openai" | "ollama" | "lmstudio" | string;

export type ProviderView = {
  id: string;
  name: string;
  kind: ProviderKind;
  baseUrl: string;
  model: string;
  hasKey: boolean;
};

export type AppSettings = {
  hotkey: string;
  historyEnabled: boolean;
  activeProviderId: string;
  clipboardContext: boolean;
  autostart: boolean;
  ttsHotkey: string;
  ttsProvider: string;
  ttsVoice: string;
  ttsModel: string;
  ttsAzureRegion: string;
  ttsCustomUrl?: string;
  ttsCustomVoice?: string;
  ttsCustomModel?: string;
  ttsGeminiVoice?: string;
  ttsGeminiModel?: string;
  ttsOpenaiVoice?: string;
  ttsOpenaiModel?: string;
  ttsElevenVoice?: string;
  ttsElevenModel?: string;
  ttsAzureVoice?: string;
  ttsAzureRegionSetting?: string;
  language?: "auto" | "en" | "de" | string;
};

export type LocalTtsStatus = {
  running: boolean;
  ready: boolean;
  url: string;
  message: string;
  deviceBackend?: string | null;
  deviceName?: string | null;
  cpuWarning?: string | null;
};

export type TtsLiveState = {
  synthesizing: boolean;
  playing: boolean;
  busy: boolean;
};

export type TtsTestPayload = {
  provider: string;
  voice: string;
  model: string;
  azureRegion: string;
  text: string;
};

export type TrayAction = "open" | "settings" | "quit";

export type ContextPayload = {
  selection: string | null;
  clipboard: string | null;
};

export type ChatSummary = {
  id: string;
  createdAt: number;
  title: string;
  providerId: string;
  model: string;
  kind?: "chat" | "tts" | string;
};

export type ChatMessage = {
  id: string;
  role: "user" | "assistant" | string;
  content: string;
  createdAt: number;
};

export type ChatDetail = {
  chat: ChatSummary;
  messages: ChatMessage[];
};

export type ChatAttachment = {
  id: string;
  name: string;
  kind: "image" | "text" | "file";
  mimeType: string;
  dataUrl?: string | null;
  textContent?: string | null;
  size?: number | null;
};

export type SessionTurn = {
  role: "user" | "assistant" | string;
  content: string;
  createdAt?: number;
  attachments?: ChatAttachment[];
};

export type Exchange = {
  id: string;
  createdAt: number;
  providerId: string;
  model: string;
  prompt: string;
  answer: string;
};
