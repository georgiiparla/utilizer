require 'base_command'

class DeleteCommand < BaseCommand
  def run
    name = @args[0] || abort("Usage: utilizer delete NAME")
    name += ".csv" unless name.end_with?(".csv")
    config = load_validated_config!(require_playlists: true)

    del_file = File.join(config["playlists_folder"], name)
    abort("Error: Playlist not found: #{del_file}") unless File.exist?(del_file)

    File.delete(del_file)
    puts "Deleted playlist: #{del_file}"
  end
end
