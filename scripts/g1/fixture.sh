#!/usr/bin/env bash
# Prints the grid fixture that the G1 gate compares screenshots against: eleven rows, then the cursor-show sequence.
# The gate clears the screen and homes the cursor before running it. Only printf is used.
# Row meanings are in the G1 definition; the expected cell values are computed by expected.py.

# Row 0: "miW." fifty times, default colours.
for _ in {1..50}; do printf 'miW.'; done
printf '\r\n'

# Row 1: sixteen ANSI background colours, two spaces each.
for n in {0..15}; do printf '\e[48;5;%dm  \e[0m' "$n"; done
printf '\r\n'

# Row 2: sixteen ANSI foreground colours, two full blocks each.
for n in {0..15}; do printf '\e[38;5;%dm██\e[0m' "$n"; done
printf '\r\n'

# Row 3: six truecolour backgrounds, two spaces each.
printf '\e[48;2;255;0;0m  \e[0m\e[48;2;0;255;0m  \e[0m\e[48;2;0;0;255m  \e[0m'
printf '\e[48;2;18;52;86m  \e[0m\e[48;2;171;205;239m  \e[0m\e[48;2;128;128;128m  \e[0m\r\n'

# Row 4: inverse video.
printf '\e[7mINVERSE\e[0m\r\n'

# Row 5: text attributes.
printf '\e[1mBOLD\e[0m \e[2mDIM\e[0m \e[3mITALIC\e[0m \e[4mUNDER\e[0m \e[4:3mCURL\e[0m \e[9mSTRIKE\e[0m\r\n'

# Row 6: six wide characters.
printf '日本語テスト\r\n'

# Rows 7 to 9: a box drawn with line characters.
printf '┌──┬──┐\r\n'
printf '│  │  │\r\n'
printf '└──┴──┘\r\n'

# Row 10: block elements and shades.
printf '█▀▄▌▐░▒▓\r\n'

printf '\e[?25h'
