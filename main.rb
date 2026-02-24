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
    utilizer config DIR_PATH    Set path to directory containing utilizer_config.json
    utilizer config             Show current config directory and contents
    utilizer list               List all available playlists
    utilizer new NAME           Create new empty playlist (e.g. vertigo)
    utilizer playlist NAME      Run a playlist (e.g. mirage.csv or mirage)
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
    warn "Error: No config directory set."
    warn "Run: utilizer config C:\\path\\to\\config\\directory"
    exit 1
  end
  unless File.exist?(config_path)
    warn "Error: Config file not found at: #{config_path}"
    warn "Create utilizer_config.json in the directory or update the path."
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
        warn "Error: Path not found: #{abs_path}"
        exit 1
      end
      unless File.directory?(abs_path)
        warn "Error: Path must be a directory, not a file: #{abs_path}"
        exit 1
      end
      config_file = File.join(abs_path, "utilizer_config.json")
      unless File.exist?(config_file)
        warn "Error: utilizer_config.json not found in: #{abs_path}"
        warn "Please create utilizer_config.json in this directory:"
        warn "#{abs_path}/utilizer_config.json"
        exit 1
      end
      save_config_path(config_file)
      puts "Config directory set to: #{abs_path}"
      puts "Using config file: #{config_file}"
    else
      config_path = load_config_path
      if config_path
        config_dir = File.dirname(config_path)
        puts "Config directory: #{config_dir}"
        puts "Config file: #{config_path}"
        if File.exist?(config_path)
          puts ""
          puts JSON.pretty_generate(load_config(config_path))
        else
          warn "(file not found)"
        end
      else
        puts "No config directory set."
        puts "Run: utilizer config C:\\path\\to\\config\\directory"
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

  when "list"
    config_path = require_config_path!
    config = load_config(config_path)

    playlists_folder = config["playlists_folder"]
    if playlists_folder.nil?
      warn "Error: playlists_folder not set in config."
      warn "Edit your config file at: #{config_path}"
      exit 1
    end
    unless Dir.exist?(playlists_folder)
      warn "Error: Playlists folder not found: #{playlists_folder}"
      exit 1
    end

    playlists = Dir.glob(File.join(playlists_folder, "*.csv")).map { |f| File.basename(f) }.sort
    if playlists.empty?
      puts "No playlists found in: #{playlists_folder}"
    else
      puts "Available playlists in: #{playlists_folder}"
      puts ""
      playlists.each { |p| puts "  #{p}" }
    end

  when "new"
    playlist_name = ARGV[1]
    unless playlist_name
      warn "Error: Playlist name required."
      warn "Usage: utilizer new NAME"
      exit 1
    end
    playlist_name += ".csv" unless playlist_name.end_with?(".csv")

    config_path = require_config_path!
    config = load_config(config_path)

    playlists_folder = config["playlists_folder"]
    if playlists_folder.nil?
      warn "Error: playlists_folder not set in config."
      warn "Edit your config file at: #{config_path}"
      exit 1
    end
    unless Dir.exist?(playlists_folder)
      warn "Error: Playlists folder not found: #{playlists_folder}"
      exit 1
    end

    new_file = File.join(playlists_folder, playlist_name)
    if File.exist?(new_file)
      warn "Error: Playlist already exists: #{new_file}"
      exit 1
    end

    File.write(new_file, "command,description\n")
    puts "Created new playlist: #{new_file}"
    puts "Open the file and add your positions in CSV format:"
    puts "  command,description"
    puts "  setpos x y z;setang pitch yaw roll,Position name"

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
