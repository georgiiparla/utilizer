require "csv"
require_relative "linked_list"
require_relative "node"

class PlaylistParser
  def initialize(playlists_folder)
    @playlists_folder = playlists_folder
  end

  def apply(filename, linked_list)
    csv_table_obj = CSV.read(File.join(@playlists_folder, filename), headers: true)
    csv_table_obj.each do |row|
      # Skip rows where command is empty or nil
      next if row["command"].nil? || row["command"].strip.empty?

      props = props_from_row(row)
      linked_list.add(props) if props
    end
  end

  private

  def props_from_row(row)
    setpos_str, setang_str = row["command"].split(";")
    
    return nil unless setpos_str && setang_str

    # Parse setpos
    _, x_str, y_str, z_str = setpos_str.split(" ")
    setpos = {
      x: x_str.to_f,
      y: y_str.to_f,
      z: z_str.to_f,
    }

    # Parse setang
    _, pitch_str, yaw_str, roll_str = setang_str.split(" ")
    setang = {
      pitch: pitch_str.to_f,
      yaw: yaw_str.to_f,
      roll: roll_str.to_f,
    }

    {
      setpos: setpos,
      setang: setang,
      description: row["description"],
      metadata: {
        raw_source: row["command"] + "," + row["description"],
      },
    }
  end
end
