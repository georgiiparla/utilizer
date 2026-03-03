require 'base_command'
require 'listener'

class ListenCommand < BaseCommand
  def run
    config = load_validated_config!
    attach_author = @args.include?("--author")
    Listener.new(config, attach_author: attach_author).run
  end
end
