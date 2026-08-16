# frozen_string_literal: true

require "optparse"
require "colorize"

class Utilizer
  Spot = Struct.new(:description, :command, :yaw, keyword_init: true)
  NUMBER = /[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?/

  class Error < StandardError; end

  def self.run(arguments)
    new(arguments).run
  rescue Error, OptionParser::ParseError, SystemCallError, EncodingError => e
    warn "Error: #{e.message}".red
    1
  end

  def initialize(arguments)
    @arguments = arguments.dup
    @output_name = nil
    @help = false
  end

  def run
    parse_options!
    return show_help if @help

    input_path = input_path!
    output_path = output_path!
    reject_same_file!(input_path, output_path)

    spots = parse_input(input_path)
    File.write(output_path, generate_cfg(spots), encoding: "UTF-8")

    puts "Created #{output_path.cyan} (#{spots.length} spots)".green
    0
  end

  private

  def parser
    @parser ||= OptionParser.new do |options|
      options.banner = "Usage: utilizer INPUT [-o NAME | --output NAME]"
      options.separator ""
      options.separator "Generate one circular CS2 playlist cfg from a text file."
      options.on("-o", "--output NAME", "Output path (default: utilizer.cfg)") { |name| @output_name = name }
      options.on("-h", "--help", "Show this help") { @help = true }
    end
  end

  def parse_options!
    parser.parse!(@arguments)
  end

  def show_help
    puts parser
    0
  end

  def input_path!
    raise Error, "missing input file\n\n#{parser}" if @arguments.empty?
    raise Error, "expected one input file, got #{@arguments.length}" if @arguments.length > 1

    path = File.expand_path(@arguments.first)
    raise Error, "input file not found: #{path}" unless File.file?(path)

    path
  end

  def output_path!
    name = @output_name || "utilizer.cfg"
    raise Error, "output name cannot be empty" if name.strip.empty?

    name = "#{name}.cfg" unless name.match?(/\.cfg\z/i)
    path = File.expand_path(name, Dir.pwd)
    directory = File.dirname(path)
    raise Error, "output directory not found: #{directory}" unless Dir.exist?(directory)

    path
  end

  def reject_same_file!(input_path, output_path)
    same_path = input_path.casecmp?(output_path)
    same_file = File.exist?(output_path) && File.identical?(input_path, output_path)
    raise Error, "input and output must be different files" if same_path || same_file
  end

  def parse_input(path)
    content = File.read(path, encoding: "bom|utf-8")
    spots = []
    errors = []

    content.each_line.with_index(1) do |line, line_number|
      line = line.strip
      next if line.empty?

      begin
        spots << parse_line(line)
      rescue Error => e
        errors << "line #{line_number}: #{e.message}"
      end
    end

    errors << "input contains no spots" if spots.empty? && errors.empty?
    raise Error, "invalid input:\n  #{errors.join("\n  ")}" unless errors.empty?

    spots
  end

  def parse_line(line)
    delimiter = line.rindex(/,\s*(?=setpos(?:\s|\z))/i)
    raise Error, "expected 'description, setpos X Y Z; setang P Y R'" unless delimiter

    description = line[0...delimiter].strip
    command = line[(delimiter + 1)..].strip
    raise Error, "description cannot be empty" if description.empty?

    statements = command.split(";", -1).map(&:strip)
    unless (1..2).cover?(statements.length)
      raise Error, "expected setpos optionally followed by setang"
    end

    position = parse_statement(statements[0], "setpos", 3)
    angles = parse_angles(statements[1]) if statements.length == 2

    Spot.new(
      description: sanitize_description(description),
      command: ["setpos #{position.join(" ")}", angles && "setang #{angles.join(" ")}"].compact.join("; "),
      yaw: angles&.[](1)
    )
  end

  def parse_angles(statement)
    parts = statement.split(/\s+/)
    unless (3..4).cover?(parts.length) && parts.first&.casecmp?("setang")
      raise Error, "expected setang with 2 or 3 numeric arguments"
    end

    angles = parts.drop(1)
    validate_numbers!(angles, "setang")
    angles << "0" if angles.length == 2
    angles
  end

  def parse_statement(statement, expected_name, argument_count)
    parts = statement.split(/\s+/)
    unless parts.length == argument_count + 1 && parts.first&.casecmp?(expected_name)
      raise Error, "expected #{expected_name} with #{argument_count} numeric arguments"
    end

    arguments = parts.drop(1)
    validate_numbers!(arguments, expected_name)

    arguments
  end

  def validate_numbers!(arguments, statement_name)
    valid = arguments.all? do |argument|
      argument.match?(/\A#{NUMBER}\z/) && Float(argument).finite?
    rescue ArgumentError
      false
    end
    raise Error, "#{statement_name} arguments must be finite numbers" unless valid
  end

  def sanitize_description(description)
    description.gsub(/\s+/, " ").tr(';"', "''")
  end

  def generate_cfg(spots)
    total = spots.length
    lines = ["sv_cheats 1"]

    spots.each_with_index do |spot, offset|
      index = offset + 1
      next_index = index == total ? 1 : index + 1
      previous_index = index == 1 ? total : index - 1
      commands = [
        "bind rightarrow ut_s#{next_index}",
        "bind leftarrow ut_s#{previous_index}",
        "bind uparrow ut_s#{index}",
        "bind downarrow ut_v#{index}",
        spot.command,
        "say_team [#{index}/#{total}] #{spot.description}"
      ]

      lines << %(alias ut_s#{index} "#{commands.join("; ")}")
      look_down = spot.yaw ? "setang 89 #{spot.yaw} 0" : ""
      lines << %(alias ut_v#{index} "#{look_down}")
    end

    lines.concat([
                   "bind rightarrow ut_s1",
                   "bind leftarrow ut_s#{total}",
                   "bind uparrow ut_s1",
                   "bind downarrow ut_v1",
                   "echo Utilizer: #{total} spots loaded"
                 ])

    "#{lines.join("\n")}\n"
  end
end

exit if defined?(Ocran)

exit Utilizer.run(ARGV)
