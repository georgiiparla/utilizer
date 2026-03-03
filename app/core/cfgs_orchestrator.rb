class CfgsOrchestrator
  def initialize(csgo_cfg_folder)
    @csgo_cfg_folder = csgo_cfg_folder
  end
  
  def create_pos_cfg_files_from_linked_list(linked_list)
    # Clean entire utilizer folder
    utilizer_dir = File.join(@csgo_cfg_folder, "utilizer")
    Dir.glob(File.join(utilizer_dir, "*")).each do |file|
      File.delete(file) if File.file?(file)
    end
    
    current = linked_list.head
    (1..linked_list.length).each do |i|
      filename = File.join(utilizer_dir, "p_#{i}.cfg")
      File.open(filename, "w") do |file|
        file.puts "noclip; setpos #{current.props[:setpos][:x]} #{current.props[:setpos][:y]} #{current.props[:setpos][:z]};setang #{current.props[:setang][:pitch]} #{current.props[:setang][:yaw]} #{current.props[:setang][:roll]}; noclip 0"
        
        loc = current.props[:location].to_s.strip
        desc = current.props[:description]
        
        file.puts "say_team \"[Utilizer] Pos #{i} (From: #{loc}) - #{desc}\""
      end
      current = current.next_node
    end
  end

  def generate_loader(linked_list)
    loader_filename = File.join(@csgo_cfg_folder, "u.cfg")
    File.open(loader_filename, "w") do |file|
      file.puts "sv_cheats 1"

      # Generate tp aliases
      (1..linked_list.length).each do |i|
        next_i = i == linked_list.length ? 1 : i + 1
        prev_i = i == 1 ? linked_list.length : i - 1
        file.puts "alias tp#{i} \"exec utilizer/p_#{i}.cfg;alias next tp#{next_i};alias prev tp#{prev_i};alias snap snap#{i};alias lookdown look#{i}\""
      end

      # Generate snap aliases
      (1..linked_list.length).each do |i|
        file.puts "alias snap#{i} \"tp#{i}\""
      end

      # Generate lookdown aliases, the setand should be taken from actual linked list node but translated to setang 89 and 0
      current = linked_list.head
      (1..linked_list.length).each do |i|
        file.puts "alias look#{i} \"setang 89 #{current.props[:setang][:yaw]} 0\""
        current = current.next_node
      end

      # Start with the first position
      file.puts "tp1"

      # Bind keys for navigation
      file.puts "bind rightarrow next"
      file.puts "bind leftarrow prev"
      file.puts "bind uparrow snap"
      file.puts "bind downarrow lookdown"
    end
  end
end
