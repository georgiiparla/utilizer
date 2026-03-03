module ChatCommandHandler
  # Processes a Chat message.
  # Returns [true, :action_symbol] if it's a command, e.g., [true, :undo] or [true, :unknown]
  # Returns [false, nil] if it's not a command message (doesn't start with "u.")
  def self.process(message)
    return [false, nil] unless message.start_with?("u.")

    command = message[2..-1].strip.downcase

    case command
    when "undo"
      [true, :undo]
    else
      [true, :unknown]
    end
  end
end
