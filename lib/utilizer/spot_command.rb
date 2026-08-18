# frozen_string_literal: true

module Utilizer
  class SpotCommand
    DECIMAL_CHARACTERS = "0123456789+-.eE"

    def self.position_statement?(text)
      first_word = text.to_s.split.first
      first_word&.casecmp?("setpos") || false
    end

    attr_reader :yaw

    def initialize(text)
      statements = split_statements(text)
      validate_statement_count!(statements)

      @position_coordinates = parse_position(statements.first)
      @view_angles = parse_view_angles(statements[1]) if statements.length == 2
      @yaw = @view_angles&.[](1)
    end

    def to_s
      statements = ["setpos #{@position_coordinates.join(" ")}"]
      statements << "setang #{@view_angles.join(" ")}" if @view_angles
      statements.join("; ")
    end

    private

    def split_statements(text)
      text.split(";").map(&:strip)
    end

    def validate_statement_count!(statements)
      return if statements.length.between?(1, 2)

      raise Error, "expected setpos optionally followed by setang"
    end

    def parse_position(statement)
      command_name, *arguments = statement.split
      unless command_name&.casecmp?("setpos") && arguments.length == 3
        raise Error, "expected setpos with 3 numeric arguments"
      end

      validate_numbers!(arguments, "setpos")
      arguments
    end

    def parse_view_angles(statement)
      command_name, *arguments = statement.split
      unless command_name&.casecmp?("setang") && arguments.length.between?(2, 3)
        raise Error, "expected setang with 2 or 3 numeric arguments"
      end

      validate_numbers!(arguments, "setang")
      arguments << "0" if arguments.length == 2
      arguments
    end

    def validate_numbers!(arguments, command_name)
      return if arguments.all? { |argument| finite_decimal?(argument) }

      raise Error, "#{command_name} arguments must be finite numbers"
    end

    def finite_decimal?(text)
      return false unless text.each_char.all? { |character| DECIMAL_CHARACTERS.include?(character) }

      number = Float(text, exception: false)
      number&.finite? || false
    end
  end
end
