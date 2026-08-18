# frozen_string_literal: true

module Utilizer
  module SpotParser
    EXPECTED_FORMAT_MESSAGE = "expected 'description, setpos X Y Z; setang P Y R'"

    extend self

    def parse(text)
      description, command_text = description_and_command(text)
      raise Error, "description cannot be empty" if description.empty?

      spot_command = SpotCommand.new(command_text)
      Spot.new(
        description: safe_description(description),
        command_text: spot_command.to_s,
        yaw: spot_command.yaw
      )
    end

    private

    def description_and_command(text)
      description, separator, command_text = text.rpartition(",")
      unless separator == "," && SpotCommand.position_statement?(command_text)
        raise Error, EXPECTED_FORMAT_MESSAGE
      end

      [description.strip, command_text.strip]
    end

    def safe_description(description)
      description.split.join(" ").gsub(";", "'").gsub('"', "'")
    end
  end
end
