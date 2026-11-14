# Where the built-in terminal colour schemes come from

Each scheme's colour values (background, foreground, cursor, selection and the 16 ANSI colours) are copied
from the file below, at the commit given, under that file's licence. No code is copied. A scheme whose source
gives no selection colour uses the colour a quarter of the way from its background to its foreground; one with
no cursor colour draws the cursor in its foreground. `scripts/terminal-schemes.py` rebuilds `schemes.json` and
this table from the source files.

| Scheme | Source | Commit | Licence | Notes |
|---|---|---|---|---|
| xterm | https://github.com/Gogh-Co/Gogh/blob/fd715c810bb0ad35f630bd73fb71f5b5a1311bd6/themes/Xterm.yml | fd715c810bb0ad35f630bd73fb71f5b5a1311bd6 | MIT OR Apache-2.0 | xterm's 16 colours (XTerm-col.ad) |
| GNOME Terminal | https://github.com/Gogh-Co/Gogh/blob/fd715c810bb0ad35f630bd73fb71f5b5a1311bd6/themes/Gnome%20Terminal.yml | fd715c810bb0ad35f630bd73fb71f5b5a1311bd6 | MIT OR Apache-2.0 | the GNOME palette of GNOME Terminal and Ptyxis |
| Konsole Breeze | https://github.com/Gogh-Co/Gogh/blob/fd715c810bb0ad35f630bd73fb71f5b5a1311bd6/themes/Breeze.yml | fd715c810bb0ad35f630bd73fb71f5b5a1311bd6 | MIT OR Apache-2.0 | Konsole's default Breeze scheme |
| Campbell | https://github.com/microsoft/terminal/blob/f6de9e0618015def1c9dda5b82f9d3abf03eb799/src/cascadia/TerminalSettingsModel/defaults.json | f6de9e0618015def1c9dda5b82f9d3abf03eb799 | MIT | Windows Terminal's default |
| macOS Terminal | https://github.com/Gogh-Co/Gogh/blob/fd715c810bb0ad35f630bd73fb71f5b5a1311bd6/themes/Terminal%20App.yml | fd715c810bb0ad35f630bd73fb71f5b5a1311bd6 | MIT OR Apache-2.0 | Terminal.app's default colours |
| iTerm2 | https://github.com/Gogh-Co/Gogh/blob/fd715c810bb0ad35f630bd73fb71f5b5a1311bd6/themes/iTerm2%20Default.yml | fd715c810bb0ad35f630bd73fb71f5b5a1311bd6 | MIT OR Apache-2.0 | iTerm2's default |
| Alacritty | https://github.com/alacritty/alacritty/blob/aa5d942f46c40184a3f214830ff5f7989f4733a1/alacritty/src/config/color.rs | aa5d942f46c40184a3f214830ff5f7989f4733a1 | Apache-2.0 | Alacritty's default |
| Kitty | https://github.com/Gogh-Co/Gogh/blob/fd715c810bb0ad35f630bd73fb71f5b5a1311bd6/themes/Kitty%20Default.yml | fd715c810bb0ad35f630bd73fb71f5b5a1311bd6 | MIT OR Apache-2.0 | kitty's default |
| WezTerm | https://github.com/wezterm/wezterm/blob/e1eae81a56be8b5487dc77a29616afa568fb57b7/term/src/color.rs | e1eae81a56be8b5487dc77a29616afa568fb57b7 | MIT | WezTerm's default |
| Ghostty | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Ghostty%20Default%20Style%20Dark.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT | Ghostty's default |
| Ayu Mirage | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Ayu%20Mirage.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| Catppuccin Latte | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Catppuccin%20Latte.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| Catppuccin Mocha | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Catppuccin%20Mocha.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| Dracula | https://github.com/dracula/iterm/blob/e5749bcd07764ad6e154038892b118ea45f88cc0/Dracula.itermcolors | e5749bcd07764ad6e154038892b118ea45f88cc0 | MIT |  |
| Everforest Dark | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Everforest%20Dark%20Hard.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT | the hard variant |
| Everforest Light | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Everforest%20Light%20Med.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT | the medium variant |
| GitHub Dark | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/GitHub%20Dark%20Default.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| GitHub Light | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/GitHub%20Light%20Default.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| Gruvbox Dark | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Gruvbox%20Dark.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| Gruvbox Light | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Gruvbox%20Light.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| Kanagawa | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Kanagawa%20Wave.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT | Kanagawa Wave, the default |
| Kanagawa Dragon | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Kanagawa%20Dragon.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| Material | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Material.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| Monokai | https://github.com/Gogh-Co/Gogh/blob/fd715c810bb0ad35f630bd73fb71f5b5a1311bd6/themes/Monokai.yml | fd715c810bb0ad35f630bd73fb71f5b5a1311bd6 | MIT OR Apache-2.0 |  |
| Night Owl | https://github.com/Gogh-Co/Gogh/blob/fd715c810bb0ad35f630bd73fb71f5b5a1311bd6/themes/Night%20Owl.yml | fd715c810bb0ad35f630bd73fb71f5b5a1311bd6 | MIT OR Apache-2.0 |  |
| Nord | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Nord.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| Oceanic Next | https://github.com/Gogh-Co/Gogh/blob/fd715c810bb0ad35f630bd73fb71f5b5a1311bd6/themes/Oceanic%20Next.yml | fd715c810bb0ad35f630bd73fb71f5b5a1311bd6 | MIT OR Apache-2.0 |  |
| One Dark | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Atom%20One%20Dark.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT | Atom's One Dark |
| One Light | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Atom%20One%20Light.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT | Atom's One Light |
| Rosé Pine | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Rose%20Pine.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| Rosé Pine Moon | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Rose%20Pine%20Moon.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| Rosé Pine Dawn | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Rose%20Pine%20Dawn.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| Solarized Dark | https://github.com/altercation/solarized/blob/62f656a02f93c5190a8753159e34b385588d5ff3/iterm2-colors-solarized/Solarized%20Dark.itermcolors | 62f656a02f93c5190a8753159e34b385588d5ff3 | MIT |  |
| Solarized Light | https://github.com/altercation/solarized/blob/62f656a02f93c5190a8753159e34b385588d5ff3/iterm2-colors-solarized/Solarized%20Light.itermcolors | 62f656a02f93c5190a8753159e34b385588d5ff3 | MIT |  |
| Tokyo Night | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/TokyoNight%20Night.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | Apache-2.0 |  |
| Tokyo Night Storm | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/TokyoNight%20Storm.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | Apache-2.0 |  |
| Tokyo Night Moon | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/TokyoNight%20Moon.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | Apache-2.0 |  |
| Tokyo Night Day | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/TokyoNight%20Day.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | Apache-2.0 |  |
| Tomorrow Night | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Tomorrow%20Night.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT |  |
| Zenburned | https://github.com/mbadolato/iTerm2-Color-Schemes/blob/95705098082c5d07fad152c31f020d50d38e10e8/schemes/Zenburned.itermcolors | 95705098082c5d07fad152c31f020d50d38e10e8 | MIT | zenbones' Zenburn |
