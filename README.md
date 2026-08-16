# Utilizer

Utilizer turns a simple spot list into one self-contained circular CS2 playlist config.

## Input

The input filename and extension do not matter. Put one spot on each nonblank line:

```text
smoke ct, setpos -512.000000 1696.000000 88.755035;setang -30.877075 -101.814423 0.000000
```

Descriptions may contain commas. Every nonblank line must contain exactly one `setpos` followed by one `setang`.

## Usage

```bat
utilizer spots.txt
utilizer spots.csv --output instant_smokes
utilizer spots -o instant_smokes.cfg
```

Output is written in the current terminal directory and always replaces an existing file. The default output is `utilizer.cfg`; `.cfg` is appended when omitted.

Put the generated config in the CS2 `game\csgo\cfg` folder, then run `exec utilizer` or the selected output name in the game console. Right and Left move through spots circularly, Up replays the current spot, and Down looks straight down while retaining its yaw.

To call `utilizer` from any terminal, add the directory containing `utilizer.exe` to your Windows `PATH`. CMD and PowerShell provide normal file path completion when you press Tab.

## Build

From this folder on Windows:

```bat
bundle install
bundle exec aibika utilizer.rb --output build/utilizer.exe --console
```
