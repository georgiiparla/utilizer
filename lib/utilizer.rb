# frozen_string_literal: true

require "colorize"

module Utilizer
  class Error < StandardError; end

  Spot = Struct.new(:description, :command_text, :yaw, keyword_init: true)

  def self.run(arguments)
    CLI.new(arguments).run
  rescue Error, OptionParser::ParseError, SystemCallError, EncodingError => e
    warn "Error: #{e.message}".red
    1
  end
end

require_relative "utilizer/spot_command"
require_relative "utilizer/spot_list_reader"
require_relative "utilizer/spot_parser"
require_relative "utilizer/spot_list_parser"
require_relative "utilizer/config_generator"
require_relative "utilizer/cli"
