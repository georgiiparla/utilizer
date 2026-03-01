require 'fileutils'
require 'csv'

class Listener
  attr_reader :config

  def initialize(config)
    @config = config
    @positions = []
    @current_context = nil
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

    File.open(console_log_path, "r") do |file|
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

        # Check for setpos output
        if coord_match = line.match(/(setpos [^;]+;setang [-\d\.]+ [-\d\.]+ [-\d\.]+)/)
          # Verify we recently executed save.cfg
          if buffer.any? { |l| l.match?(/execing save\.cfg/) }
            coord = coord_match[1]
            @positions << { coord: coord, context: @current_context }
            puts "[Captured] #{coord}"
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
    File.foreach(log_path) do |line|
      update_context(line.strip, verbose: false)
    end
    if @current_context
      puts "[Attached Context] #{@current_context}"
    end
  end

  def update_context(line, verbose: true)
    if match = line.match(/\[Demo\] playing demo from '(.+?)'/)
      name = match[1].gsub('.dem', '')
      if @current_context != name
        @current_context = name
        puts "[Context changed] #{@current_context}" if verbose
      end
    elsif match = line.match(/\[Client\] Map: "(.+?)"/)
      name = match[1]
      if name != "<empty>" && @current_context != name
        @current_context = name
        puts "[Context changed] #{@current_context}" if verbose
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
    
    input = STDIN.gets
    input = input ? input.strip : ""
    filename = input.empty? ? default_name : input
    filename += ".csv" unless filename.end_with?(".csv")

    playlists_folder = @config["playlists_folder"]
    full_path = File.join(playlists_folder, filename)

    is_new = !File.exist?(full_path)
    
    CSV.open(full_path, "a") do |csv|
      csv << ["command", "description"] if is_new
      @positions.each_with_index do |pos_data, i|
        desc = "Spot #{i + 1}"
        desc += " (#{pos_data[:context]})" if pos_data[:context]
        csv << [pos_data[:coord], desc]
      end
    end

    puts "Saved -> #{full_path}"
  end
end
