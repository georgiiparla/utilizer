# Utilizer - CS2 Position Playlist Loader

## Why Utilizer?
Utilizer provides a unique, elegant, client-side approach to saving and loading CS2 positions without requiring dedicated servers, plugins, or paid subscriptions.

- **Live Console Listener:** Eliminates manual config writing by tailing `console.log` in real-time to capture coordinates directly.
- **CSV Playlists:** Organizes positions in highly readable, easily shareable, and mergeable CSV files instead of messy alias scripts.
- **Pure Client-Side Cfg:** A lightweight, standalone CLI that instantly generates dynamic `.cfg` routines with zero server setup required.

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

3. Add these exact launch options to your Counter-Strike 2 in Steam:
```
-condebug -conclearlog
```

## Usage

```
utilizer                    Load the most recently edited playlist
utilizer NAME               Load a playlist (e.g. utilizer mirage)
utilizer listen             Listen to console to capture spots (requires save.cfg)
utilizer list               List all playlists
utilizer new NAME           Create new playlist
utilizer merge L1 L2 NEW_L  Merge two playlists into a new one
utilizer delete NAME        Delete a playlist
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

Type `exec u` in console to activate.
