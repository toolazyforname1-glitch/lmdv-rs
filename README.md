# lmdv-tui

A terminal Markdown reader and editor inspired by [lmdv](https://github.com/LeandroMarceddu/lmdv-rs). It keeps the reading card, outline, file browser, themes, editor and settings of the desktop app, but runs in the terminal with Ratatui.

## Build and run

```sh
cargo build --release
target/release/lmdv-tui path/to/document.md
```

On Windows, `lmdv-tui.bat` launches the release binary. It also accepts dragged-and-dropped `.md` files or a folder.

## Usage

```
lmdv-tui [FILES...] [OPTIONS]
  --theme <black|dark|light|sepia>   Start in this theme (default: System)
  --no-motion                        Disable entrance and scroll animation
  --browser                          Open the file browser pane
  --newtab                           Open a New tab start page
  --editor                           Open the current document in the editor
  --snapshot                         Render one frame headlessly and print it
  --width <COLS> --height <ROWS>     Snapshot size
  --scroll <LINE>                    Snapshot/editor starting line
  --help                             Show usage
```

With no arguments the start page lists recent files and the Markdown files in the working directory.

## Reading

- `j`/`k` or arrows: scroll one line; `d`/`u`: half page; `space`: page down; `g`/`G`: top/bottom.
- Mouse wheel: scroll. Motion over a pane divider thickens it; drag it to resize the browser, outline or editor split.
- `o` toggles the left file browser; `Tab` toggles the right outline. Click a folder to expand it and a file to open it.
- `/` opens Find; `n`/`N` move between matches. The find bar has case-sensitive and whole-word toggles.
- Click a task checkbox to tick it; the source file is written with its original line endings preserved.
- Code blocks have a `copy` chip; the Share menu (`share` toolbar chip) offers Copy text/path/link and Export HTML.
- `[` and `]` switch tabs; click the `x` on a tab (or press `x`) to close it; `Ctrl+T` creates a New tab start page; `Ctrl+Shift+T` reopens a closed tab.

## Editor

`Ctrl+E` (or the `edit` toolbar chip) toggles the editor. Wide terminals show a source pane and a live rendered preview side by side; the divider is draggable and the preview follows the source scroll position.

Editing commands: `Ctrl+S` save, `Ctrl+Z`/`Ctrl+Y` undo/redo, `Ctrl+B`/`Ctrl+I`/`Ctrl+K` bold/italic/link, `Alt+Up`/`Alt+Down` move line, `Ctrl+D` duplicate line, `Ctrl+Shift+K` delete line, `F2` toggle preview. The format bar exposes the same actions by mouse. Closing a dirty tab asks to save or discard.

## Settings and state

`Ctrl+,` (or the `settings` toolbar chip) opens Settings: theme, text size, font, spacing, column width, side panels, outline, restore session, motion, print page break, format bar, large-file mode, spellcheck, language, reset, and a Windows “make default for Markdown files” action. Dragged pane widths are persisted too.

Preferences live in `%APPDATA%\lmdv-tui\settings.json`; tabs and reading positions in `%APPDATA%\lmdv-tui\session.json` on Windows. The default handler action writes only per-user registry keys and opens Windows Default Apps for the final confirmation.

## Help

Press `?` for the in-app key reference, or use the command palette with `Ctrl+P`.
