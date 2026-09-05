# Terminal

The **Terminal** panel is a real, interactive command line living inside the
editor. It's not a log view that echoes a few commands — it's a genuine shell
(PowerShell on Windows, your `$SHELL` elsewhere) running through an operating-
system pseudo-terminal. That means full-screen, interactive programs work
exactly as they do in a standalone terminal: you can run `claude`, `vim`,
`htop`, `git`, a dev server — anything.

> **Terminal vs. Console.** The **Console** panel shows the engine's own log
> messages (info / warnings / errors, with filters and a slash-command box). The
> **Terminal** panel is a separate thing: an actual shell for running programs.

## Opening it

The Terminal is a distribution plugin, so it shows up like any other panel:

1. Click the **`+`** (Add Panel) button on a dock and pick **Terminal** (under
   the *Tools* category), **or** drag the **Terminal** tab into any dock split.
2. The shell starts the first time the panel becomes visible.

If you don't see it in the picker, the `renzora_terminal` plugin isn't present —
it ships as a removable dylib in `plugins/`. Delete that file and the editor has
no terminal at all.

## Using it

- **Click the terminal** to give it keyboard focus (a block cursor appears).
  While it's focused, your keystrokes go to the shell — editor shortcuts like
  `G`/`S`/`Delete` and camera movement are suppressed so typing isn't hijacked.
- **Click elsewhere** (or switch tabs) to release focus and get your editor
  shortcuts back.
- Everything you'd expect works: **Enter**, **Backspace**, **Tab** completion,
  arrow-key history, **Ctrl-C** to interrupt, **Ctrl-D** to send EOF, and the
  full arrow/Home/End/PageUp/PageDown set for TUIs.
- **Resizing** the panel resizes the shell — programs re-flow to the new width
  and height automatically.
- **Scroll** with the mouse wheel to look back through output history; typing
  jumps you back to the live bottom.
- **Select** text by clicking and dragging. Copy it with **Ctrl+Shift+C** and
  paste with **Ctrl+Shift+V** (the *Shift* keeps plain **Ctrl+C** free to send an
  interrupt to the running program).

### Running Claude

Because it's a true terminal, you can launch interactive assistants right in the
editor:

```text
claude
```

The full-screen interface renders inside the panel, colors and all.

## Choosing a different shell

By default the Terminal launches PowerShell on Windows and your login shell
(`$SHELL`, falling back to `/bin/bash`) on Linux and macOS. To override it, set
the `RENZORA_TERMINAL_SHELL` environment variable before launching the editor —
for example `RENZORA_TERMINAL_SHELL=cmd.exe` or `RENZORA_TERMINAL_SHELL=zsh`.

The shell starts in the editor's current working directory.

## Good to know

- The shell is killed when the panel is destroyed (and hangs up when the editor
  exits).
- Output keeps flowing while the tab is hidden, so a long-running command — or a
  backgrounded `claude` session — is still there when you switch back to it.
- The terminal is **not** available in the exported game; it's an editor-only
  panel.
