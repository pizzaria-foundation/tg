# Changelog

## v0.2.0 — 2026-08-26

Telegram client for Symbian S60, speaking MTProto directly.

- **Chats, messages and contacts**, with the cryptography implemented here — there is
  no third-party library for this platform.
- **Registered as an MTM**, so it appears in the phone's native Inbox alongside SMS.
- **Follows the phone's theme**.

**Install:** download `telegram.sis` and open it on the device. Connecting needs
Telegram application credentials in `api.conf`. Unsigned, so it needs an unlocked
installserver (Open4All / RomPatcher+).
