# Utilizer - CS2 Position Playlist Loader

## Installation

1. Download `utilizer.exe`
2. Add exe folder to PATH (Environment Variables)
3. Run `utilizer help`

## Setup

1. Create folder with `utilizer_config.json`:
```json
{
  "playlists_folder": "C:/your/playlists/folder",
  "csgo_cfg_folder": "C:/Program Files (x86)/Steam/steamapps/common/Counter-Strike Global Offensive/game/csgo/cfg"
}
```

2. Set config path:
```
utilizer config C:\path\to\config\folder
```

## Usage

```
utilizer list               List all playlists
utilizer new NAME           Create new playlist
utilizer playlist NAME      Run a playlist
```

## Playlist Format

CSV with `command` and `description`:
```
command,description
setpos x y z;setang pitch yaw roll,Position name
```

## In-Game

- **Right Arrow** - Next position
- **Left Arrow** - Previous position
- **Up Arrow** - Snap position
- **Down Arrow** - Look down

Type `load` in console to activate.
