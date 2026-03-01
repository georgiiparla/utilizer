require 'base_command'

class ExecuteCommand < BaseCommand
  def run
    if @args.empty?
      handle_latest
    else
      handle_playlist(@args[0])
    end
  end

  private

  def handle_latest
    config = load_validated_config!(require_playlists: true)
    csv_files = Dir.glob(File.join(config["playlists_folder"], "*.csv"))
    
    if csv_files.empty?
      puts "No playlists found in #{config["playlists_folder"]}"
      exit 0
    end

    latest_file = csv_files.max_by { |f| File.mtime(f) }
    run_and_report(config, File.basename(latest_file), "Loaded latest")
  end

  def handle_playlist(name)
    name += ".csv" unless name.end_with?(".csv")
    config = load_validated_config!
    run_and_report(config, name, "Loaded")
  end
end
