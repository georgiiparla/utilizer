$LOAD_PATH.unshift File.join(__dir__, "app")
$LOAD_PATH.unshift File.join(__dir__, "app", "core")
$LOAD_PATH.unshift File.join(__dir__, "app", "commands")

require "playlist_parser"
require "linked_list"
require "cfgs_orchestrator"
require "listener"
require "cli"

CLI.new(ARGV).run
