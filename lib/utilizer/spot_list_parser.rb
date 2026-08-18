# frozen_string_literal: true

module Utilizer
  class SpotListParser
    def initialize(input_path)
      @source_reader = SpotListReader.new(input_path)
    end

    def parse
      spots = []
      error_messages = []

      @source_reader.read.each do |source_line|
        spots << SpotParser.parse(source_line.text)
      rescue Error => e
        error_messages << "line #{source_line.line_number}: #{e.message}"
      end

      error_messages << "input contains no spots" if spots.empty? && error_messages.empty?
      raise Error, combined_error_message(error_messages) unless error_messages.empty?

      spots
    end

    private

    def combined_error_message(error_messages)
      "invalid input:\n  #{error_messages.join("\n  ")}"
    end
  end
end
