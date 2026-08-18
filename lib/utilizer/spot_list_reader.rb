# frozen_string_literal: true

module Utilizer
  class SpotListReader
    class SourceLine
      attr_reader :text, :line_number

      def initialize(text, line_number)
        @text = text.strip
        @line_number = line_number
      end

      def ignored?
        text.empty? || text.start_with?("//")
      end

      def unfinished_description?
        text.end_with?(",")
      end

      def command_continuation?
        SpotCommand.position_statement?(text)
      end

      def continued_with(other)
        self.class.new("#{text} #{other.text}", line_number)
      end
    end

    def initialize(input_path)
      @input_path = input_path
    end

    def read
      logical_lines = []
      unfinished_line = nil

      physical_lines.each do |line|
        next if line.ignored?

        if unfinished_line
          if line.command_continuation?
            logical_lines << unfinished_line.continued_with(line)
            unfinished_line = nil
            next
          end

          logical_lines << unfinished_line
          unfinished_line = nil
        end

        if line.unfinished_description?
          unfinished_line = line
        else
          logical_lines << line
        end
      end

      logical_lines << unfinished_line if unfinished_line
      logical_lines
    end

    private

    def physical_lines
      source_lines = []
      content = File.read(@input_path, encoding: "bom|utf-8")
      content.each_line.with_index(1) do |text, line_number|
        source_lines << SourceLine.new(text, line_number)
      end
      source_lines
    end
  end
end
