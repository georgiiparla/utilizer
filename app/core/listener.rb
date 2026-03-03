require 'fileutils'
require 'csv'
require_relative 'chat_command_handler'

class Listener
  attr_reader :config

  def initialize(config, attach_author: false)
    @config = config
    @attach_author = attach_author
    @positions = []
    @current_context = nil
    @current_player = nil
    @mode = nil
    @pending_coord = nil
  end

  def run
    console_log_path = find_console_log
    unless File.exist?(console_log_path)
      warn "Error: console.log not found at #{console_log_path}"
      exit 1
    end

    # Read from beginning to find the most recent map/demo context
    find_initial_context(console_log_path)
    
    puts "Listening to CS2 console (Ctrl+C to stop)..."

    # Setup trap for Ctrl+C
    interrupted = false
    trap("INT") do
      interrupted = true
    end

    File.open(console_log_path, "r", encoding: 'UTF-8', invalid: :replace, undef: :replace) do |file|
      file.seek(0, IO::SEEK_END) # Start tailing from the end
      
      buffer = []
      
      while !interrupted
        line = file.gets
        if line.nil?
          sleep 0.05
          next
        end

        line = line.strip
        
        # Check for context changes
        update_context(line)

        buffer << line
        buffer.shift if buffer.size > 5

        # Check for map mode chat messages
        if @mode == :map && @current_player && line.include?(@current_player)
          if match = line.match(/\[(?:T|CT|ALL|Spec)\] (.*?):\s+(.*)$/)
            prefix = match[1]
            msg = match[2].strip
            next if msg == "[Utilizer] Description:" || msg.empty?

            # Handle u.<command> syntax
            is_command, action = ChatCommandHandler.process(msg)
            if is_command
              case action
              when :undo
                if @pending_coord
                  @pending_coord = nil
                  puts "[Undid] Pending coordinate dropped."
                elsif @positions.any?
                  dropped = @positions.pop
                  puts "[Undid] Removed spot: #{dropped[:coord]}"
                else
                  puts "[Undid] Nothing to undo!"
                end
              when :unknown
                puts "[Warning] Unknown Utilizer command: #{msg}"
              end
              buffer.clear
              next
            end

            # Extract location from prefix (e.g. Sławek Masuma﹫Apartments)
            # The character is U+FE6B (SMALL COMMERCIAL AT) or standard @
            location = nil
            if loc_match = prefix.match(/(?:﹫|@)(.+)$/)
              location = loc_match[1].strip
            end

            # Otherwise, use message as description for pending coord
            if @pending_coord
              @positions << { coord: @pending_coord, context: @current_context, description: msg, location: location }
              puts "[Captured] #{@pending_coord} | Loc: #{location || '...'} | Desc: #{msg}"
              @pending_coord = nil
              buffer.clear
              next
            end
          end
        end

        # Check for setpos output
        if coord_match = line.match(/(setpos [^;]+;setang [-\d\.]+ [-\d\.]+ [-\d\.]+)/)
          # Verify we recently executed save.cfg
          if buffer.any? { |l| l.match?(/execing save\.cfg/) }
            coord = coord_match[1]
            if @mode == :map
              @pending_coord = coord
              puts "[Pending] Enter description in team chat..."
            else
              @positions << { coord: coord, context: @current_context }
              puts "[Captured] #{coord}"
            end
            # Clear buffer to avoid duplicate captures
            buffer.clear
          end
        end
      end
    end

    puts "\nCaught #{@positions.size} spots."
    save_positions if @positions.any?
  end

  private

  def find_initial_context(log_path)
    File.foreach(log_path, encoding: 'UTF-8', invalid: :replace, undef: :replace) do |line|
      update_context(line.strip, verbose: false)
    end
    if @current_context
      mode_name = @mode ? @mode.to_s.capitalize : "Unknown"
      puts "[Attached Context] #{@current_context} | Mode: #{mode_name}"
    end
    if @current_player && @attach_author
      puts "[Attached Player] #{@current_player}"
    end
  end

  def update_context(line, verbose: true)
    if match = line.match(/\[Demo\] playing demo from '(.+?)'/)
      name = match[1].gsub('.dem', '')
      changed = false
      if @current_context != name
        @current_context = name
        changed = true
      end
      if @mode != :demo
        @mode = :demo
        changed = true
      end
      puts "[Context] #{@current_context} | [Mode] Demo" if changed && verbose
    elsif match = line.match(/\[Client\] Map: "(.+?)"/)
      name = match[1]
      if name != "<empty>"
        changed = false
        if @current_context != name
          @current_context = name
          changed = true
        end
        if @mode != :map
          @mode = :map
          changed = true
        end
        puts "[Context] #{@current_context} | [Mode] Map" if changed && verbose
      end
    elsif match = line.match(/ClientPutInServer create new player controller \[(.+?)\]/)
      player_name = match[1]
      if @current_player != player_name
        @current_player = player_name
        puts "[Player Detected] #{@current_player}" if verbose
      end
    elsif match = line.match(/Added prediction for player slot \d+ \((.+?)\)/)
      player_name = match[1]
      if @current_player != player_name
        @current_player = player_name
        puts "[Player Detected] #{@current_player}" if verbose
      end
    end
  end

  def find_console_log
    cfg_folder = @config["csgo_cfg_folder"]
    parent = File.expand_path("..", cfg_folder)
    log_path = File.join(parent, "console.log")
    
    return log_path if File.exist?(log_path)

    "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Counter-Strike Global Offensive\\game\\csgo\\console.log"
  end

  def save_positions
    trap("INT", "DEFAULT")

    default_name = @current_context ? "#{@current_context}_spots.csv" : "spots.csv"
    print "Playlist name to save [#{default_name}]: "
    
    begin
      input = STDIN.gets
    rescue Interrupt
      puts "\nCancelled saving."
      return
    end

    input = input ? input.strip : ""
    filename = input.empty? ? default_name : input
    filename += ".csv" unless filename.end_with?(".csv")

    playlists_folder = @config["playlists_folder"]
    full_path = File.join(playlists_folder, filename)

    is_new = !File.exist?(full_path)
    
    CSV.open(full_path, "a") do |csv|
      csv << ["command", "description", "location"] if is_new
      @positions.each_with_index do |pos_data, i|
        desc = pos_data[:description] || "Spot #{i + 1}"
        desc += " (#{pos_data[:context]})" if pos_data[:context]
        desc += " [Author: #{@current_player}]" if @attach_author && @current_player
        loc = pos_data[:location] || ""
        csv << [pos_data[:coord], desc, loc]
      end
    end

    puts "Saved -> #{full_path}"
  end
end
