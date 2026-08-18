# frozen_string_literal: true

module Utilizer
  class ConfigGenerator
    def initialize(spots)
      @spots = spots
    end

    def generate
      config_lines = ["sv_cheats 1"]
      @spots.each.with_index(1) do |spot, index|
        config_lines.concat aliases_for(spot, index)
      end
      config_lines.concat startup_commands

      "#{config_lines.join("\n")}\n"
    end

    private

    def aliases_for(spot, index)
      playback_commands = [
        "bind rightarrow util#{next_index(index)}",
        "bind leftarrow util#{previous_index(index)}",
        "bind uparrow util#{index}",
        "bind downarrow ut_v#{index}",
        spot.command_text,
        "say_team [#{index}/#{spot_count}] #{spot.description}"
      ]

      [
        %(alias util#{index} "#{playback_commands.join("; ")}"),
        %(alias ut_v#{index} "#{look_down_command(spot)}")
      ]
    end

    def startup_commands
      [
        "bind rightarrow util1",
        "bind leftarrow util#{spot_count}",
        "bind uparrow util1",
        "bind downarrow ut_v1",
        "echo Utilizer: #{spot_count} spots loaded"
      ]
    end

    def next_index(index)
      index == spot_count ? 1 : index + 1
    end

    def previous_index(index)
      index == 1 ? spot_count : index - 1
    end

    def look_down_command(spot)
      spot.yaw ? "setang 89 #{spot.yaw} 0" : ""
    end

    def spot_count
      @spots.length
    end
  end
end
