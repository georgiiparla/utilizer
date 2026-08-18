# frozen_string_literal: true

require "optparse"

module Utilizer
  class CLI
    DEFAULT_OUTPUT_NAME = "utilizer.cfg"

    def initialize(arguments)
      @arguments = arguments.dup
      @output_name = nil
      @help_requested = false
    end

    def run
      parse_options!
      return show_help if @help_requested

      input_path = input_path!
      output_path = output_path!
      reject_same_file!(input_path, output_path)

      spots = SpotListParser.new(input_path).parse
      File.write(output_path, ConfigGenerator.new(spots).generate, encoding: "UTF-8")

      puts "Created #{output_path.cyan} (#{spots.length} spots)".green
      0
    end

    private

    def option_parser
      @option_parser ||= OptionParser.new do |options|
        options.banner = "Usage: utilizer INPUT [-o NAME | --output NAME]"
        options.separator ""
        options.separator "Generate one circular CS2 playlist cfg from a text file."
        options.on("-o", "--output NAME", "Output path (default: #{DEFAULT_OUTPUT_NAME})") do |output_name|
          @output_name = output_name
        end
        options.on("-h", "--help", "Show this help") { @help_requested = true }
      end
    end

    def parse_options!
      option_parser.parse!(@arguments)
    end

    def show_help
      puts option_parser
      0
    end

    def input_path!
      raise Error, "missing input file\n\n#{option_parser}" if @arguments.empty?
      raise Error, "expected one input file, got #{@arguments.length}" if @arguments.length > 1

      input_path = File.expand_path(@arguments.first)
      raise Error, "input file not found: #{input_path}" unless File.file?(input_path)

      input_path
    end

    def output_path!
      output_name = @output_name || DEFAULT_OUTPUT_NAME
      raise Error, "output name cannot be empty" if output_name.strip.empty?

      output_name = "#{output_name}.cfg" unless output_name.downcase.end_with?(".cfg")
      output_path = File.expand_path(output_name, Dir.pwd)
      output_directory = File.dirname(output_path)
      raise Error, "output directory not found: #{output_directory}" unless Dir.exist?(output_directory)

      output_path
    end

    def reject_same_file!(input_path, output_path)
      same_path = input_path.casecmp?(output_path)
      same_file = File.exist?(output_path) && File.identical?(input_path, output_path)
      raise Error, "input and output must be different files" if same_path || same_file
    end
  end
end
