require 'base_command'

class NewCommand < BaseCommand
  def run
    name = @args[0] || abort("Usage: utilizer new NAME")
    name += ".csv" unless name.end_with?(".csv")
    config = load_validated_config!(require_playlists: true)

    new_file = File.join(config["playlists_folder"], name)
    abort("Error: Playlist already exists: #{new_file}") if File.exist?(new_file)

    File.write(new_file, "command,description\n")
    puts "Created playlist: #{new_file}"
  end
end
