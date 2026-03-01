require 'base_command'

class ListCommand < BaseCommand
  def run
    config = load_validated_config!(require_playlists: true)
    playlists = Dir.glob(File.join(config["playlists_folder"], "*.csv")).map { |f| File.basename(f) }.sort
    
    if playlists.empty?
      puts "No playlists found in #{config["playlists_folder"]}"
    else
      puts "Available playlists:"
      playlists.each { |p| puts "  #{p}" }
    end
  end
end
