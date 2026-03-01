require 'config_command'
require 'list_command'
require 'listen_command'
require 'new_command'
require 'delete_command'
require 'execute_command'
require 'merge_command'

class CLI
  USAGE = <<~HELP
    Utilizer - CS2 position playlist loader

    Usage:
      utilizer                    Run the most recently edited playlist
      utilizer NAME               Run a playlist (e.g. utilizer mirage)
      utilizer listen             Listen to CS2 console log to record position spots
      utilizer list               List all available playlists
      utilizer new NAME           Create new empty playlist (e.g. utilizer new vertigo)
      utilizer merge L1 L2 NEW_L  Merge two playlists into a new one
      utilizer delete NAME        Delete a playlist
      utilizer config DIR_PATH    Set path to directory containing utilizer_config.json
      utilizer config             Show current config directory and contents
      utilizer help               Show this help
  HELP

  COMMAND_MAP = {
    "config" => ConfigCommand,
    "list"   => ListCommand,
    "listen" => ListenCommand,
    "new"    => NewCommand,
    "merge"  => MergeCommand,
    "delete" => DeleteCommand
  }

  def initialize(args)
    @command = args[0]
    @args = args[1..-1] || []
  end

  def run
    if ["help", "-h", "--help"].include?(@command)
      puts USAGE
      return
    end

    if @command.nil? || !COMMAND_MAP.key?(@command)
      # No command provided (latest) or unrecognized command (assume playlist name)
      ExecuteCommand.new([@command].compact).run
    else
      cmd_class = COMMAND_MAP[@command]
      cmd_class.new(@args).run
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
end
