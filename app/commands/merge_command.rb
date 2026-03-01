require 'base_command'

class MergeCommand < BaseCommand
  def run
    source1 = @args[0]
    source2 = @args[1]
    dest = @args[2]

    unless source1 && source2 && dest
      abort("Usage: utilizer merge LIST1 LIST2 NEW_LIST")
    end

    source1 += ".csv" unless source1.end_with?(".csv")
    source2 += ".csv" unless source2.end_with?(".csv")
    dest += ".csv" unless dest.end_with?(".csv")

    config = load_validated_config!(require_playlists: true)
    playlists_folder = config["playlists_folder"]

    src1_path = File.join(playlists_folder, source1)
    src2_path = File.join(playlists_folder, source2)
    dest_path = File.join(playlists_folder, dest)

    abort("Error: Playlist not found: #{src1_path}") unless File.exist?(src1_path)
    abort("Error: Playlist not found: #{src2_path}") unless File.exist?(src2_path)
    abort("Error: Destination playlist already exists: #{dest_path}") if File.exist?(dest_path)

    # Read the lines, omitting the header from the second file, or from both and manually writing it
    lines1 = File.readlines(src1_path).map(&:chomp)
    lines2 = File.readlines(src2_path).map(&:chomp)

    # Standardize header
    header = "command,description"
    
    # Remove headers if they exist in the source files
    lines1.shift if lines1.first == header
    lines2.shift if lines2.first == header

    # Ensure no purely blank lines at the very beginning of the files (just in case)
    lines1.shift while lines1.first&.empty?
    lines2.shift while lines2.first&.empty?

    # Add an empty line between the two files to keep them visually separated
    combined = [header] + lines1 + [""] + lines2

    File.write(dest_path, combined.join("\n") + "\n")
    puts "Merged #{source1} and #{source2} into #{dest}"
    puts "Total spots: #{lines1.size + lines2.size}"
  end
end
