<div align="center">
  <img src="static/splash-icon.png" alt="Dumbo Logo" width="96" />
  <h1>Dumbo</h1>
  <p>
    <a href="https://github.com/Lolaplex/dumbo/releases"><img src="https://img.shields.io/badge/version-0.45.0-blue.svg?style=flat-square" alt="Version 0.45.0" /></a>
    <img src="https://img.shields.io/badge/Windows-10%20%2F%2011-0078D4.svg?style=flat-square&logo=windows&logoColor=white" alt="Windows 10/11" />
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" alt="License" /></a>
  </p>
  <p>
    <strong>Your AI, one keypress away.</strong><br>
    Ask questions, dictate text and have answers read aloud — in any Windows app, without switching windows.
  </p>
</div>

---

## What is Dumbo?

Dumbo is a small helper that lives in your Windows system tray. Press a key, a small box appears, you type or speak, and the answer is there. Press the key again and it is gone.

It works on top of whatever you are doing. You do not have to open a browser, copy text back and forth, or leave the app you are working in.

Dumbo does not bring its own AI. You connect it to an AI service you already use (or want to use) with an account and an API key. That keeps you in control of cost and privacy.

---

## What you can do with it

### Ask anything — `Alt+Space`

A small floating box appears in the middle of your screen. Type your question and press `Enter` for a short answer, or `Shift+Enter` if you want a longer, more detailed one.

The box stays where you put it. Drag it by the handle at the top edge and Dumbo remembers that spot for next time.

### Dictate instead of typing — `Ctrl+Shift+Space`

Press the key, speak, and press it again. Dumbo writes what you said straight into the app you are using — a chat, a document, an email, a code editor. No copy-paste.

It also cleans up your speech: "um", "uh" and false starts are removed, and the text is formatted as a proper sentence or instruction.

If you want it to send the message right away, turn on **Auto-Enter after Dictation** in Settings.

> Dictation needs a free **Groq** API key. See [Voice setup](#voice-setup) below.

### Have text read aloud — `Alt+Shift+S`

Select text in any app and press the key. Dumbo reads it out loud. Press again to stop immediately.

If you have nothing selected, it reads whatever is in your clipboard. And with `Ctrl+Alt+R` it reads the last answer Dumbo gave you, without you having to select anything.

You can pick the voice and the service in Settings — Azure, ElevenLabs, Google Gemini, OpenAI, or your own local speech server.

### Ask about what you have selected

Highlight text in a browser, a PDF, Word or your editor, then press `Alt+Space`. Dumbo picks up the selection automatically and uses it as context, so you can just ask "explain this" or "what is wrong here".

### Everything is remembered

Every conversation is saved locally on your PC. Scroll back through earlier chats whenever you need them. Nothing is uploaded anywhere except to the AI service you chose.

---

## Install

1. Download the latest installer from **[Releases](https://github.com/Lolaplex/dumbo/releases)**.
2. Run it. Windows may ask for confirmation — Dumbo is not code-signed yet, so click **More info → Run anyway**.
3. Dumbo starts and waits in the system tray (the small icons next to your clock).

Windows 10 and 11 only. macOS is not supported yet.

---

## First steps

1. Right-click the Dumbo icon in the tray and choose **Settings** (or press `Ctrl+,`).
2. Under **Provider**, choose an AI service and paste your API key.
3. Press **Save**.
4. Press `Alt+Space` anywhere and ask something.

### Where do I get an API key?

You need an account with one of these services. All of them offer a free tier or free trial credit:

| Service | Good for |
|---|---|
| **Google Gemini** | Generous free tier, good all-rounder |
| **OpenAI** | Best known, reliable |
| **Anthropic Claude** | Long, careful answers |
| **OpenRouter** | One key for many different models |
| **Local / Custom** | Your own server, fully offline |

Pick one, create a key in that service's dashboard, and paste it into Dumbo. Your key is stored in the **Windows Credential Manager**, not in a plain text file.

---

## Voice setup

Dumbo's voice features are optional. Text chat works without them.

### Dictation (speech to text)

1. Create a free account at [console.groq.com](https://console.groq.com) and generate an API key (it starts with `gsk_`).
2. In Dumbo Settings, open **Speech-to-Text** and paste the key.
3. Press `Ctrl+Shift+Space`, say something, press it again.

Dumbo uses Groq's Whisper model to turn your speech into text, then a second model tidies it up. You can switch the tidy-up step off if you prefer your words exactly as spoken.

### Read aloud (text to speech)

1. In Settings, open **Text-to-Speech** and pick a service.
2. Paste that service's API key.
3. Use **Test voice** to hear a sample.

Azure gives you the widest choice of natural voices and languages. ElevenLabs sounds the most human. Gemini and OpenAI are quick and simple. **Custom** lets you point Dumbo at your own local speech server if you run one.

---

## Shortcuts

| Action | Shortcut |
|---|---|
| Open / close the ask box | `Alt+Space` |
| Short answer | `Enter` |
| Detailed answer | `Shift+Enter` |
| Dictate (speech to text) | `Ctrl+Shift+Space` |
| Read selection or clipboard aloud | `Alt+Shift+S` |
| Read Dumbo's last answer aloud | `Ctrl+Alt+R` |
| Settings | `Ctrl+,` |
| Close the ask box | `Esc` |

Every shortcut can be changed in Settings. The interface is available in **English and German**.

---

## Keeping Dumbo up to date

Dumbo checks for new versions itself. Open Settings and press **Check for updates** — if a newer version exists, it downloads and installs in one click. Updates are cryptographically signed, so nobody can slip you a modified copy.

---

## If something does not work

**Nothing happens when I press a shortcut.**
Another program may already be using that key combination. Change the shortcut in Settings.

**Dictation records nothing.**
Check that Windows allows microphone access: *Settings → Privacy & security → Microphone*. Also make sure the right microphone is selected as the default input device.

**I get an error about my API key.**
The key may be expired, or it may belong to a different service than the one selected in Settings. Create a new key and paste it again.

**No sound when reading aloud.**
Press **Test voice** in the Text-to-Speech section. If that fails, the key or the selected voice is the problem.

**The update check fails.**
Make sure you are on version 0.45.0 or newer. Older versions pointed at an update file that no longer exists.

---

## Privacy

- API keys live in the **Windows Credential Manager**.
- Chat history stays in a local database on your PC.
- Your prompts and selected text are sent only to the AI service you configured, never to us.
- Dumbo has no telemetry and no analytics.

---

## License

MIT License &copy; 2026 Lolaplex — see [LICENSE](LICENSE).
