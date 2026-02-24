require "json"
require "fileutils"

$LOAD_PATH.unshift File.join(__dir__, "app")
require "playlist_parser"
require "linked_list"
require "cfgs_orchestrator"

CONFIG_PATH_FILE = File.join(Dir.home, ".utilizer_config_path")

USAGE = <<~HELP
  Utilizer - CS2 position playlist loader

  Usage:
    utilizer config ABS_PATH   Set path to your utilizer_config.json
    utilizer config             Show current config path and contents
    utilizer playlist NAME      Run a playlist (e.g. mirage.csv)
    utilizer help               Show this help
HELP

def save_config_path(abs_path)
  File.write(CONFIG_PATH_FILE, abs_path.strip)
end

def load_config_path
  return nil unless File.exist?(CONFIG_PATH_FILE)
  path = File.read(CONFIG_PATH_FILE).strip
  path.empty? ? nil : path
end

def load_config(path)
  JSON.parse(File.read(path))
rescue JSON::ParserError => e
  raise "Invalid JSON in config: #{path} (#{e.message})"
end

def ensure_utilizer_dir(csgo_cfg_folder)
  FileUtils.mkdir_p(File.join(csgo_cfg_folder, "utilizer"))
end

def run_pipeline(config, playlist)
  playlists_folder = config.fetch("playlists_folder")
  csgo_cfg_folder = config.fetch("csgo_cfg_folder")

  ensure_utilizer_dir(csgo_cfg_folder)

  ll = LinkedList.new
  parser = PlaylistParser.new(playlists_folder)
  parser.apply(playlist, ll)

  orchestrator = CfgsOrchestrator.new(csgo_cfg_folder)
  orchestrator.create_pos_cfg_files_from_linked_list(ll)
  orchestrator.generate_loader(ll)

  ll.length
end

def require_config_path!
  config_path = load_config_path
  if config_path.nil?
    warn "Error: No config path set."
    warn "Run: utilizer config ABS_PATH"
    exit 1
  end
  unless File.exist?(config_path)
    warn "Error: Config file not found at: #{config_path}"
    warn "Create the file or update the path with: utilizer config ABS_PATH"
    exit 1
  end
  config_path
end

begin
  command = ARGV[0]

  case command
  when "config"
    abs_path = ARGV[1]
    if abs_path
      abs_path = File.expand_path(abs_path)
      unless File.exist?(abs_path)
        warn "Error: File not found: #{abs_path}"
        exit 1
      end
      save_config_path(abs_path)
      puts "Config path saved: #{abs_path}"
    else
      config_path = load_config_path
      if config_path
        puts "Config path: #{config_path}"
        if File.exist?(config_path)
          puts JSON.pretty_generate(load_config(config_path))
        else
          warn "(file not found)"
        end
      else
        puts "No config path set. Run: utilizer config ABS_PATH"
      end
    end

  when "playlist"
    playlist_name = ARGV[1]
    unless playlist_name
      warn "Error: Playlist name required."
      warn "Usage: utilizer playlist NAME"
      exit 1
    end
    playlist_name += ".csv" unless playlist_name.end_with?(".csv")

    config_path = require_config_path!
    config = load_config(config_path)

    if config["playlists_folder"].nil? || config["csgo_cfg_folder"].nil?
      warn "Error: Configuration incomplete."
      warn "Edit your config file at: #{config_path}"
      warn "Required keys: playlists_folder, csgo_cfg_folder"
      exit 1
    end

    count = run_pipeline(config, playlist_name)
    puts "Generated cfgs for #{count} positions."

  when "help", "-h", "--help"
    puts USAGE

  when nil
    puts USAGE
    exit 1

  else
    warn "Unknown command: #{command}"
    warn USAGE
    exit 1
  end
rescue KeyError => e
  warn "Missing config key: #{e.message}"
  exit 1
rescue Errno::ENOENT => e
  warn "Not found: #{e.message}"
  exit 1
rescue StandardError => e
  warn e.message
  exit 1
end
