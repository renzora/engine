# Terminal

The **Terminal** panel is a real, interactive command line living inside the
editor. It is not a log view that echoes a few commands: it is a genuine shell
(PowerShell on Windows, your `$SHELL` elsewhere) running through an operating-
system pseudo-terminal. That means full-screen, interactive programs work
exactly as they do in a standalone terminal, so you can run `claude`, `vim`,
`htop`, `git add -p`, a dev server, anything.

> **Terminal vs. Console.** The **Console** panel shows the engine's own log
> messages (info / warnings / errors, with filters and a slash-command box). The
> **Terminal** panel is a separate thing: an actual shell for running programs.

## Opening it

The Terminal is a [native plugin](../extending/native-plugins.md), so it shows up
like any other panel:

1. Click the **`+`** (Add Panel) button on a dock and pick **Terminal** (under
   the *Tools* category), **or** drag the **Terminal** tab into any dock split.
2. The shell starts the first time the panel becomes visible, once the panel has
   a size to start it at.

If you do not see it in the picker, the `terminal` plugin is not installed. It
lives in `plugins/terminal/` beside the editor; delete that directory, or switch
it off in **Settings → Editor → Plugins**, and the editor has no terminal at all.

## Tabs

The strip is one chip per shell. **`+`** opens another, the **`x`** on a chip
closes that one, and clicking a chip switches to it.

`+` sits at the end of the last tab in a top strip, where the next one will
appear. In a side strip it joins the controls at the top instead, just left of
the two layout toggles: below a column of chips it would drift further from
everything else with every tab opened.

- **Ctrl+Shift+T** opens a tab, **Ctrl+Shift+W** closes the current one.
- **Ctrl+Tab** and **Ctrl+Shift+Tab** move forward and back through them.
- Each tab is a separate shell with its own history, its own scrollback and its
  own selection. Switching is instant: nothing is torn down and nothing restarts.
- **Background tabs keep running.** A build in tab 2 carries on while you work in
  tab 1, and its output is waiting when you switch back.
- Closing a tab kills its shell. Closing the *last* tab opens a fresh one rather
  than leaving the panel empty, so "close the only tab" reads as "give me a clean
  shell".
- A chip is named after the program it is running plus its number (`bash 1`,
  `zsh 2`). A tab whose shell has exited keeps its name and shows the exit notice
  until you press Enter.

All tabs share the panel's size, so resizing the panel resizes every one of
them, not just the one on screen. A background tab that kept an old width would
re-flow the moment it came forward, which is the shell repainting a screen it
drew for a size it no longer has.

### Renaming

**Double-click** a chip, or **right-click** it and pick **Rename**. The name
becomes an editable field with the old one selected, so typing replaces it.
**Enter** or clicking away commits; **Escape** cancels.

A name you type is kept. Restarting a dead shell with Enter re-names an
*automatic* chip after whatever started, but never one you have renamed
yourself: an automatic name is a placeholder, a typed one is a decision.

The right-click menu also has **New Tab**, **Close** and **Close Others**.

### When they do not fit

Chips **give ground before they clip**. Open more of them across the top and they
narrow, browser-style, down to a floor of a few characters plus the close button.
That alone absorbs a lot: a strip that comfortably shows four tabs at full width
shows eight squeezed.

Past that floor they genuinely overflow, and two things handle it.

The **active tab is scrolled into view**, so pressing `+` on a full strip shows
you the tab you just made rather than appearing to do nothing.

A **`v`** appears at the end of the strip. It lists every tab, with a tick on the
one you are looking at, and clicking one switches to it. It is the way to reach a
tab that is currently scrolled out of sight.

It carries a **New Tab** row too, because a top strip's `+` rides at the end of
the tab list and has therefore been clipped away with them. (A side strip's is
in the header band, which never clips, and Ctrl+Shift+T never moves either way.)

It appears from measuring the laid-out chips, not from counting them, so it is
right whatever the theme's font does to a chip's width.

### Top or side

The two buttons at the end of the strip put the tabs **across the top** or **down
the right**. Side tabs stack, so they are the ones to reach for when the panel is
tall and narrow, or when you have enough shells open that a row of them clips.

The choice lasts for the session. A plugin has no slot in the editor's settings
file, and a terminal layout is cheap to pick again.

## Using it

- **Click the terminal** to give it keyboard focus (the block cursor fills in).
  While it is focused your keystrokes go to the shell, and editor shortcuts like
  `G`/`S`/`Delete` and the camera keys are held off so typing is not hijacked.
- **Click elsewhere** (or switch tabs) to release focus and get your editor
  shortcuts back. An unfocused terminal still shows a dimmed cursor, so you can
  see where the shell is without it looking like it is taking input.
- Everything you would expect works: **Enter**, **Backspace**, **Tab**
  completion, arrow-key history, **Ctrl-C** to interrupt, **Ctrl-D** to send EOF,
  **Alt+B** / **Alt+F** to move by word, and the full arrow / Home / End /
  PageUp / PageDown / F1-F12 set for TUIs.
- **Resizing** the panel resizes the shell. The grid is refitted to whatever
  number of whole rows and columns the panel holds, and the new size is pushed
  into the kernel, which is what makes a full-screen program re-flow.
- **Scroll** with the mouse wheel to look back through history (10,000 lines of
  it), or drag the **scrollbar** down the right-hand edge. Typing jumps you back
  to the live bottom.
- The scrollbar appears once something has scrolled off the top, and the whole
  track is grabbable, not only the thumb: pressing anywhere on it jumps to that
  point in the history. Its gutter is always reserved, even when there is nothing
  to scroll, because the width it would otherwise take back is the terminal's
  **column count** and the first line of output would resize the shell.
- **Select** text by clicking and dragging. Copy it with **Ctrl+Shift+C** and
  paste with **Ctrl+Shift+V**.

The *Shift* on copy and paste is not a stylistic choice: plain **Ctrl+C** has to
stay the interrupt, or there is no way to stop a runaway command.

### Running Claude

Because it is a true terminal, you can launch interactive assistants right in the
editor:

```text
claude
```

The full-screen interface renders inside the panel, colours and all.

### When the shell exits

Type `exit`, or kill the shell, and the panel says so and stops. Press **Enter**
to start a new one in the same panel.

## Choosing a different shell

By default the Terminal launches PowerShell on Windows and your login shell
(`$SHELL`, falling back to `/bin/bash`) on Linux and macOS. To override it, set
the `RENZORA_TERMINAL_SHELL` environment variable before launching the editor,
for example `RENZORA_TERMINAL_SHELL=cmd.exe` or `RENZORA_TERMINAL_SHELL=zsh`.

The shell starts in the **project directory** when a project is open, and in the
editor's own working directory otherwise. A terminal in a game editor is
overwhelmingly used to run something against the project, and where the launcher
happened to start the editor is rarely that.

`TERM` is set to `xterm-256color` and `COLORTERM` to `truecolor`, so a program
can use the full palette without probing.

## Good to know

- The shell is killed when the panel is destroyed, and when the editor exits.
  Closing the master would eventually hang the far end up on its own, but a
  program ignoring `SIGHUP` would survive as an orphan with no terminal and no
  way to reach it.
- **Output keeps flowing while the panel is hidden**, and in every terminal tab,
  not just the one on screen. A long build, or a backgrounded `claude` session,
  is still running and still there when you come back. This is not politeness: a
  pseudo-terminal nobody reads fills its kernel buffer and blocks the program
  writing into it, so pausing would suspend the command rather than background
  it.
- Colours follow the editor theme for the default foreground and background, and
  a fixed 256-colour palette for everything a program asks for by name. A
  terminal in a light theme is dark-on-light without the shell being told.
- The terminal is **not** available in the exported game. It is an editor-only
  panel, and a shell in a shipped game's own process is not a feature.
- Bold does not render as a heavier weight (the panel has one monospace face).
  It does still brighten the eight indexed colours to their bright counterparts,
  which is what a prompt asking for "bold red" is actually after.

## For plugin authors

The Terminal is a small, complete example of the things a native plugin panel
usually has to do: measuring a monospace font, fitting a grid to a
`ComputedNode`, driving a `Text` with per-span colours and backgrounds without
rebuilding it, taking the keyboard away from the editor's shortcuts, running
background threads whose output has to survive the panel being hidden, and
carrying a small tab strip that is rebuilt on change rather than bound. Its
source is in `plugins/terminal/`, six modules and about 1,200 lines.

See [Native Plugins](../extending/native-plugins.md),
[Editor Panels from a Plugin](../extending/panels.md), and
[Taking the keyboard away from the shortcuts](../extending/editor-api.md#taking-the-keyboard-away-from-the-shortcuts).
