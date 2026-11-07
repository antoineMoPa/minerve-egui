# Agents

- Test UI changes headless only: `minerve-gui --screenshot out.png [--screenshot-after-ms N] [--submit TEXT]`.
  Never open a visible window: it steals focus from whoever is typing.
- Talk to minerve only through the control socket. Never copy minerve code into this repo; if the GUI needs
  something from minerve, add it to the protocol (an event or a command listed in `hello.features`).
- Don't add to the README. Humans will manage it. Feel free to fix broken info, but don't add.
