# Utilizer

Utilizer turns a simple spot list into one self-contained circular CS2 playlist config.

## Input

The input filename and extension do not matter. Put one spot on each nonblank line:

```text
smoke ct, setpos -512.000000 1696.000000 88.755035;setang -30.877075 -101.814423 0.000000
```

Descriptions may contain commas. Every nonblank line must contain one `setpos`. The
`setang` portion is optional, and its optional third (roll) value defaults to `0`:

```text
CT spawn 1, setpos -512 1696 88.755043
CT elbow, setpos -512 1696 88.842270;setang -26.949894 -101.927994
```

The description and command may also be split after the comma, with optional blank lines between them:

```text
CT elbow,
setpos -512 1696 88.842270;setang -26.949894 -101.927994
```

For a position-only spot, replaying it does not change the view angle and the Down
key does nothing because there is no saved yaw to preserve.

## Usage

```bat
utilizer spots.txt
utilizer spots.csv --output instant_smokes
utilizer spots -o instant_smokes.cfg
```

Output is written in the current terminal directory and always replaces an existing file. The default output is `utilizer.cfg`; `.cfg` is appended when omitted.

Put the generated config in the CS2 `game\csgo\cfg` folder, then run `exec utilizer` or the selected output name in the game console. Right and Left move through spots circularly, Up replays the current spot, and Down looks straight down while retaining its yaw. Enter `util1`, `util2`, and so on to jump directly to a numbered spot.

To call `utilizer` from any terminal, add the directory containing `utilizer.exe` to your Windows `PATH`. CMD and PowerShell provide normal file path completion when you press Tab.

## Build

From this folder on Windows:

```bat
bundle install
bundle exec ocran --output build/utilizer.exe --console --icon logo.ico utilizer.rb
```
