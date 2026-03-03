require 'base_command'

class ConfigCommand < BaseCommand
  def run
    abs_path = @args[0]
    if abs_path == "populate"
      config = load_validated_config!
      cfg_folder = config["csgo_cfg_folder"]
      save_cfg_path = File.join(cfg_folder, "save.cfg")
      
      File.write(save_cfg_path, "sv_cheats 1;\ngetpos;\nsay_team \"[Utilizer] Description:\"\n")
      puts "Successfully populated save.cfg in #{cfg_folder}"
    elsif abs_path
      abs_path = File.expand_path(abs_path)
      unless File.directory?(abs_path)
        warn "Error: Path must be a directory: #{abs_path}"
        exit 1
      end
      config_file = File.join(abs_path, "utilizer_config.json")
      unless File.exist?(config_file)
        warn "Error: utilizer_config.json not found in #{abs_path}"
        exit 1
      end
      File.write(CONFIG_PATH_FILE, config_file)
      puts "Config directory set to: #{abs_path}"
    else
      config_path = load_config_path
      if config_path
        puts "Config file: #{config_path}"
        puts JSON.pretty_generate(load_config(config_path)) if File.exist?(config_path)
      else
        puts "No config directory set. Run: utilizer config C:\\path\\to\\config"
      end
    end
  end
end
