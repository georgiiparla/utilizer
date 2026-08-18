# frozen_string_literal: true

require_relative "lib/utilizer"

exit if defined?(Ocran)

exit Utilizer.run(ARGV)
