# Hardware test

One pass per machine, 45 minutes or less. Use the latest release from
https://github.com/ZahakJ/dioptra/releases/latest. Tick as you go; stop at the first
blocker and send it back (last section).

Bring to every machine, on a USB stick or in a synced folder:

- `excel-paste.xlsx`: three columns of numbers with a header row, one cell with a decimal
  comma, one gene name such as `SEPT2`, and an empty cell.
- two `.pzfx` files (a Column and an XY table) and one `.prism` file from
  `fixtures/prism/` in the repository.
- a trial key. Issue it the day before:

  ```sh
  cd ~/Documents/side/dioptra-keys
  printf '%s\n' '{"id":"T-2026-001","licensee":"Hardware test","email":"you@example.org",
  "plan":"trial","seats":1,"issued":"2026-10-14","expires":"2026-12-13"}' \
    | target/release/dioptra-keys issue -
  ```

Machines to cover:

| Machine | Installer | Passes |
|---|---|---|
| Windows 11 | msi, then exe | 2 |
| Windows 10 22H2 | msi | 1 |
| Mac, Apple silicon, macOS 15 or later | `_aarch64.dmg` | 1 |
| Mac, Apple silicon, macOS 13 or 14 | `_aarch64.dmg` | 1 |
| Mac, Intel | `_x64.dmg` | 1 |

Borrow machines: a university computer lab, a friend, a department loaner. A clean user
account is better than a fresh install of the OS.

## Windows (10 and 11)

Record first: `winver` build number, display scale, which installer.

Install

- [ ] Download the `.msi` (or `-setup.exe`) in Edge. Note any download warning.
- [ ] SmartScreen shows "Windows protected your PC": *More info* → *Run anyway* works.
- [ ] msi: asks for administrator rights once, installs to `C:\Program Files\Dioptra`.
- [ ] exe: installs without administrator rights, to `%LOCALAPPDATA%`.
- [ ] Start-menu entry exists; the icon is the Dioptra icon, not a blank one.

First launch

- [ ] Starts in under 5 s. No WebView2 prompt (Windows 10: if it asks to install
      WebView2, note whether the installer fetched it and whether the app then started).
- [ ] Empty project with three cards: paste, sample project, manual.
- [ ] Open the sample project. Every table, analysis and graph opens; nothing is blank.

Data in

- [ ] Open `excel-paste.xlsx` in Excel, copy the three columns, *File → Paste as new
      table* (Ctrl-Shift-V). Preview correct; decimal comma detected; `SEPT2` stays text.
- [ ] *File → Import* the same `.xlsx`. Same values.
- [ ] Open both `.pzfx` files. Tables, replicates, row titles and exclusions match Prism.
- [ ] Open the `.prism` file (if the release notes say `.prism` is read). Same check.

Analyses

- [ ] Column table: unpaired t test. Assumption checks shown, effect size with CI.
- [ ] Same table: one-way ANOVA with Tukey. Comparisons listed.
- [ ] XY table: nonlinear fit, four-parameter dose-response. Curve drawn, EC50 with CI.
- [ ] Change a value in the table; the results sheet goes stale and recomputes.

Graph

- [ ] *Make graph* from the ANOVA. Click the Y axis title, retype it.
- [ ] Switch the family to Box, then Violin; brackets follow.
- [ ] Change theme and palette. Drag a label. Undo (Ctrl-Z) and redo work.

Export

- [ ] Export PDF, SVG and PNG (300 dpi, 85 × 70 mm). The dialog's pixel size matches the file.
- [ ] PDF in Edge or Acrobat: text is selectable, font is Inter, nothing clipped.
- [ ] SVG in Inkscape: text is text (editable), not paths; font Inter or a clean fallback.
- [ ] PDF and SVG in Illustrator, if available: no missing-font warning, line weights as set.
- [ ] PNG opens in Photos at the stated size.

Save

- [ ] Ctrl-S to `Documents\test.dioptra`. Close the app. Double-click the file: it opens
      in Dioptra and every result is the same.
- [ ] Save to a path with spaces and non-ASCII characters (`Données été\test.dioptra`).

Dioptra Lab

- [ ] *Help → Dioptra Lab*. Paste the trial key, activate. Shows the licensee, plan
      "trial" and the end date.
- [ ] Quit and relaunch: still active.
- [ ] Paste a key with one character changed: refused with a clear message.

Display

- [ ] *Settings → Display → Scale* at 125 %, then 150 %: text sharp, no clipped dialogs,
      toolbar fits, the graph canvas and rulers line up with the mouse.
- [ ] Move the window to a second monitor with another scale (if available): redraws sharp.
- [ ] *Settings → Personalization → Colors → Dark*: the app follows; graph paper stays white.

Uninstall

- [ ] *Settings → Apps → Dioptra → Uninstall*. Start-menu entry gone.
- [ ] `.dioptra` files keep their data; reinstall and the key is still there (config in
      `%APPDATA%`) — note either way.

## macOS (Apple silicon and Intel; 13, 14, 15 and later)

Record first: Apple menu → *About This Mac* (chip, macOS version and build), which dmg.

Install

- [ ] Download the dmg for the chip in Safari. Open it; the window shows Dioptra and an
      Applications link.
- [ ] Drag to Applications. Eject the dmg.
- [ ] Never "is damaged and can't be opened". If it appears, stop and send it back.

First launch (Gatekeeper)

- [ ] Double-click: "Apple could not verify 'Dioptra'…". Note the exact wording.
- [ ] macOS 13–14: Control-click → *Open* → *Open* launches it.
- [ ] macOS 15+: *System Settings → Privacy & Security → Open Anyway*, password, *Open*.
- [ ] Delete the app, download and drag it again, then run only
      `xattr -cr /Applications/Dioptra.app` in Terminal: it launches with no prompt.
- [ ] Second launch from the Dock or Launchpad: no prompt.
- [ ] Intel Mac: the `_x64.dmg` runs natively (Activity Monitor → Kind: Intel). Apple
      silicon: Kind: Apple.
- [ ] Empty project with three cards. Open the sample project; everything opens.

Data in

- [ ] Copy three columns from Excel for Mac (or Numbers), ⌘-Shift-V. Same checks as Windows.
- [ ] Import the `.xlsx`; open both `.pzfx` and the `.prism` file.

Analyses

- [ ] Unpaired t test, one-way ANOVA with Tukey, four-parameter nonlinear fit. Same checks
      as Windows.

Graph

- [ ] *Make graph*, edit the axis title, switch families, theme, palette, undo/redo (⌘-Z,
      ⌘-Shift-Z).

Export

- [ ] Export PDF, SVG, PNG.
- [ ] PDF in Preview: *Tools → Show Inspector → Fonts* lists Inter, embedded.
- [ ] SVG in Inkscape and Illustrator: text editable, no missing-font warning.
- [ ] PNG in Preview: *Tools → Adjust Size* shows the stated pixels and dpi.

Save

- [ ] ⌘-S to `~/Documents/test.dioptra`, quit (⌘-Q), double-click the file in Finder: it
      opens in Dioptra with the same results.
- [ ] Save into an iCloud Drive folder; reopen.

Dioptra Lab

- [ ] *Help → Dioptra Lab*, paste the trial key, activate. Relaunch: still active.
- [ ] Altered key refused.

Display

- [ ] *System Settings → Appearance → Dark*, then *Auto*: the app follows live.
- [ ] *Displays → Larger Text* (scaled resolution) and an external non-Retina monitor if
      available: text sharp, canvas and rulers line up with the pointer.
- [ ] Full screen (⌃⌘F) and Split View work; menus are in the macOS menu bar.

Uninstall

- [ ] Drag Dioptra from Applications to the Bin. Note whether
      `~/Library/Application Support/` holds a Dioptra folder (key, settings).

## If something fails, send back

One message or GitHub issue per failure:

1. Dioptra version (*Help → About*) and installer file name.
2. OS and build: Windows `winver` (e.g. 11 23H2 22631.4317); macOS *About This Mac*
   (e.g. 15.1 24B83, M2). Display scale.
3. Steps from launch, numbered, as short as reproduces it.
4. What you expected, what happened.
5. Screenshot of the window or dialog (Win-Shift-S; ⌘-Shift-4).
6. The file you used, if it involves data and can be shared.
7. Whether it happens every time.

File it at https://github.com/ZahakJ/dioptra/issues with the label `platform`.
