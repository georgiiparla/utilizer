require 'base_command'
require 'listener'

class ListenCommand < BaseCommand
  def run
    config = load_validated_config!
    Listener.new(config).run
  end
end
