class BaseCommand
  CONFIG_PATH_FILE = File.join(Dir.home, ".utilizer_config_path")

  def initialize(args)
    @args = args || []
  end

  def run
    raise NotImplementedError, "#{self.class} must implement #run"
  end

  protected

  def load_config_path
    File.exist?(CONFIG_PATH_FILE) ? File.read(CONFIG_PATH_FILE).strip : nil
  end

  def load_config(path)
    JSON.parse(File.read(path))
  end

  def load_validated_config!(require_playlists: false)
    path = load_config_path || abort("Error: No config directory set. Run: utilizer config [dir]")
    abort("Error: Config not found at #{path}") unless File.exist?(path)
    
    config = load_config(path)
    abort("Error: Config missing playlists_folder or csgo_cfg_folder") if config["csgo_cfg_folder"].nil? || config["playlists_folder"].nil?
    abort("Error: Playlists folder not found") if require_playlists && !Dir.exist?(config["playlists_folder"])
    
    config
  end

  def run_and_report(config, playlist_name, prefix_msg)
    FileUtils.mkdir_p(File.join(config["csgo_cfg_folder"], "utilizer"))
    
    ll = LinkedList.new
    PlaylistParser.new(config["playlists_folder"]).apply(playlist_name, ll)
    
    orchestrator = CfgsOrchestrator.new(config["csgo_cfg_folder"])
    orchestrator.create_pos_cfg_files_from_linked_list(ll)
    orchestrator.generate_loader(ll)
    
    puts "#{prefix_msg}: #{playlist_name} (#{ll.length} positions)"
  end
end
